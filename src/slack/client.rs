use crate::auth::WorkspaceAuth;
use crate::error::{Result, SlackersError};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

// =========================================================================
// Request / Response types
// =========================================================================

/// Response from files.upload
#[derive(Debug, Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub id: String,
    pub name: String,
    pub title: String,
    pub permalink: Option<String>,
    pub url_private: Option<String>,
}

/// Response from conversations.open — wraps the returned channel object
#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationOpenResponse {
    /// Channel ID (use this for subsequent API calls)
    pub id: String,
    pub is_open: Option<bool>,
    /// True when the DM was already open prior to this call
    pub already_open: Option<bool>,
    /// Full channel object as returned by Slack
    pub channel: Value,
}

/// Response from chat.update
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatUpdateResponse {
    pub channel: String,
    pub ts: String,
    pub text: String,
}

/// Icon information returned as part of `team.info`
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WorkspaceIcon {
    pub image_34: Option<String>,
    pub image_44: Option<String>,
    pub image_68: Option<String>,
    pub image_88: Option<String>,
    pub image_102: Option<String>,
    pub image_132: Option<String>,
    pub image_230: Option<String>,
    pub image_default: Option<bool>,
}

/// Workspace (team) information returned by `team.info`
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub icon: Option<WorkspaceIcon>,
}

// =========================================================================

const MAX_RETRIES: u32 = 10;
const BASE_RETRY_DELAY_SECS: u64 = 5;
const MAX_RETRY_DELAY_SECS: u64 = 60;

pub struct SlackClient {
    http: reqwest::Client,
    auth: WorkspaceAuth,
    workspace_url: Option<String>,
}

impl SlackClient {
    pub fn new(auth: WorkspaceAuth, workspace_url: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http,
            auth,
            workspace_url,
        }
    }

    /// Call a Slack API method
    ///
    /// Handles both standard token and browser token modes.
    /// Retries on 429 (rate limit) with exponential backoff.
    /// Set SLACKERS_DEBUG=1 to log every call to stderr.
    pub async fn api_call(
        &self,
        method: &str,
        params: Vec<(String, String)>,
    ) -> Result<Value> {
        if std::env::var("SLACKERS_DEBUG").is_ok() {
            let args = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" ");
            eprintln!("[api] {} {}", method, args);
        }
        let mut attempt = 0;

        loop {
            attempt += 1;

            let result = match &self.auth {
                WorkspaceAuth::Standard { token } => {
                    self.api_call_standard(method, token, &params).await
                }
                WorkspaceAuth::Browser {
                    xoxc_token,
                    xoxd_cookie,
                } => {
                    self.api_call_browser(method, xoxc_token, xoxd_cookie, &params)
                        .await
                }
            };

            match result {
                Ok(response) => return Ok(response),
                Err(e) if attempt < MAX_RETRIES && is_rate_limit_error(&e) => {
                    let delay = extract_retry_delay(&e, attempt);
                    eprintln!("Rate limited; retrying in {}s (attempt {}/{})...", delay, attempt, MAX_RETRIES);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Standard token API call: POST https://slack.com/api/{method}
    async fn api_call_standard(
        &self,
        method: &str,
        token: &str,
        params: &[(String, String)],
    ) -> Result<Value> {
        let url = format!("https://slack.com/api/{}", method);

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|e| SlackersError::Other(e.to_string()))?,
        );

        let response = self
            .http
            .post(&url)
            .headers(headers)
            .form(params)
            .send()
            .await?;

        let status = response.status();
        let body: Value = response.json().await?;

        if status.as_u16() == 429 {
            // Extract retry-after header if present
            // Will be handled by retry logic
            return Err(SlackersError::from_slack_api(
                "Rate limited".to_string(),
                Some("rate_limited".to_string()),
            ));
        }

        Self::check_api_response(body)
    }

    /// Browser token API call: POST {workspace_url}/api/{method}
    async fn api_call_browser(
        &self,
        method: &str,
        xoxc_token: &str,
        xoxd_cookie: &str,
        params: &[(String, String)],
    ) -> Result<Value> {
        let workspace_url = self
            .workspace_url
            .as_ref()
            .ok_or_else(|| SlackersError::Other("Workspace URL required for browser auth".to_string()))?;

        let url = format!("{}/api/{}", workspace_url, method);

        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!("d={}", urlencoding::encode(xoxd_cookie)))
                .map_err(|e| SlackersError::Other(e.to_string()))?,
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://app.slack.com"),
        );

        // Add xoxc token to params
        let mut browser_params = params.to_vec();
        browser_params.insert(0, ("token".to_string(), xoxc_token.to_string()));

        let response = self
            .http
            .post(&url)
            .headers(headers)
            .form(&browser_params)
            .send()
            .await?;

        let status = response.status();
        let body: Value = response.json().await?;

        if status.as_u16() == 429 {
            return Err(SlackersError::from_slack_api(
                "Rate limited".to_string(),
                Some("rate_limited".to_string()),
            ));
        }

        Self::check_api_response(body)
    }

    // =========================================================================
    // Task 2.1: files.upload
    // =========================================================================

    /// Upload a file to Slack using multipart/form-data.
    ///
    /// `file_path`       – local path of the file to upload
    /// `channels`        – list of channel IDs to share the file into (may be empty)
    /// `initial_comment` – optional message to accompany the file
    /// `title`           – optional display title in Slack
    /// `filename`        – overrides the on-disk filename if provided
    pub async fn upload_file(
        &self,
        file_path: &Path,
        channels: Vec<String>,
        initial_comment: Option<String>,
        title: Option<String>,
        filename: Option<String>,
    ) -> Result<FileUploadResponse> {
        use reqwest::multipart;

        // Read file bytes
        let file_bytes = std::fs::read(file_path)
            .map_err(|e| SlackersError::Other(format!("Failed to read file '{}': {}", file_path.display(), e)))?;

        // Determine filename to send
        let upload_filename = filename
            .clone()
            .or_else(|| {
                file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "upload".to_string());

        // Detect MIME type from extension (basic heuristic)
        let mime_type = detect_mime_type(&upload_filename);

        // Build multipart form
        let file_part = multipart::Part::bytes(file_bytes)
            .file_name(upload_filename.clone())
            .mime_str(mime_type)
            .map_err(|e| SlackersError::Other(format!("Invalid MIME type: {}", e)))?;

        let mut form = multipart::Form::new().part("file", file_part);

        form = form.text("filename", upload_filename);

        if !channels.is_empty() {
            form = form.text("channels", channels.join(","));
        }
        if let Some(comment) = initial_comment {
            form = form.text("initial_comment", comment);
        }
        if let Some(t) = title {
            form = form.text("title", t);
        }

        // Build request with auth headers
        let url = "https://slack.com/api/files.upload";
        let mut headers = HeaderMap::new();

        match &self.auth {
            WorkspaceAuth::Standard { token } => {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", token))
                        .map_err(|e| SlackersError::Other(e.to_string()))?,
                );
            }
            WorkspaceAuth::Browser { xoxc_token, xoxd_cookie } => {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", xoxc_token))
                        .map_err(|e| SlackersError::Other(e.to_string()))?,
                );
                headers.insert(
                    COOKIE,
                    HeaderValue::from_str(&format!("d={}", urlencoding::encode(xoxd_cookie)))
                        .map_err(|e| SlackersError::Other(e.to_string()))?,
                );
                headers.insert(
                    ORIGIN,
                    HeaderValue::from_static("https://app.slack.com"),
                );
            }
        }

        let response = self
            .http
            .post(url)
            .headers(headers)
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        let body: Value = response.json().await?;

        if status.as_u16() == 429 {
            return Err(SlackersError::from_slack_api(
                "Rate limited".to_string(),
                Some("rate_limited".to_string()),
            ));
        }

        let body = Self::check_api_response(body)?;

        let file = body
            .get("file")
            .ok_or_else(|| SlackersError::Other("No 'file' field in upload response".to_string()))?;

        let upload_response = FileUploadResponse {
            id: file.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            permalink: file.get("permalink").and_then(|v| v.as_str()).map(|s| s.to_string()),
            url_private: file.get("url_private").and_then(|v| v.as_str()).map(|s| s.to_string()),
            name: file.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: file.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        Ok(upload_response)
    }

    // =========================================================================
    // Task 2.2: files.delete
    // =========================================================================

    /// Delete a file by ID.
    ///
    /// Returns an error with a descriptive message for `file_not_found` and
    /// `cant_delete_file` error codes; all others propagate as-is.
    pub async fn delete_file(&self, file_id: &str) -> Result<()> {
        let params = vec![("file".to_string(), file_id.to_string())];
        match self.api_call("files.delete", params).await {
            Ok(_) => Ok(()),
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "file_not_found" =>
            {
                Err(SlackersError::Other(format!(
                    "File '{}' not found or already deleted",
                    file_id
                )))
            }
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "cant_delete_file" =>
            {
                Err(SlackersError::Other(format!(
                    "Cannot delete file '{}': permission denied or file belongs to another user",
                    file_id
                )))
            }
            Err(e) => Err(e),
        }
    }

    // =========================================================================
    // Task 7.1: conversations.open
    // =========================================================================

    /// Open or retrieve a direct message / group DM conversation.
    ///
    /// Pass one or more Slack user IDs in `users`.  Returns the channel object
    /// containing at minimum an `id` field that can be used for subsequent
    /// `chat.postMessage` and other channel-scoped calls.
    pub async fn open_conversation(&self, users: Vec<String>) -> Result<ConversationOpenResponse> {
        let users_str = users.join(",");
        let params = vec![("users".to_string(), users_str)];
        let body = self.api_call("conversations.open", params).await?;

        let channel = body
            .get("channel")
            .ok_or_else(|| SlackersError::Other("No 'channel' field in conversations.open response".to_string()))?;

        let response = ConversationOpenResponse {
            id: channel
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SlackersError::Other("Missing channel id in conversations.open response".to_string()))?
                .to_string(),
            is_open: channel.get("is_open").and_then(|v| v.as_bool()),
            already_open: body.get("already_open").and_then(|v| v.as_bool()),
            channel: channel.clone(),
        };

        Ok(response)
    }

    // =========================================================================
    // Task 8.1: pins.add / pins.remove
    // =========================================================================

    /// Pin a message to a channel.
    ///
    /// `channel` – channel ID that contains the message
    /// `ts`      – message timestamp (the unique message ID in Slack)
    ///
    /// Handles `already_pinned` gracefully by returning Ok(()).
    pub async fn pin_message(&self, channel: &str, ts: &str) -> Result<()> {
        let params = vec![
            ("channel".to_string(), channel.to_string()),
            ("timestamp".to_string(), ts.to_string()),
        ];
        match self.api_call("pins.add", params).await {
            Ok(_) => Ok(()),
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "already_pinned" =>
            {
                // Already pinned — treat as success
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Unpin a message from a channel.
    ///
    /// `channel` – channel ID that contains the message
    /// `ts`      – message timestamp
    ///
    /// Handles `not_pinned` gracefully by returning Ok(()).
    pub async fn unpin_message(&self, channel: &str, ts: &str) -> Result<()> {
        let params = vec![
            ("channel".to_string(), channel.to_string()),
            ("timestamp".to_string(), ts.to_string()),
        ];
        match self.api_call("pins.remove", params).await {
            Ok(_) => Ok(()),
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "not_pinned" =>
            {
                // Not pinned — treat as success (idempotent)
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    // =========================================================================
    // Task 8.2: chat.delete
    // =========================================================================

    /// Delete a message.
    ///
    /// Note: bots can only delete their own messages unless the token has admin
    /// permissions.
    ///
    /// Returns descriptive errors for `cant_delete_message` and
    /// `message_not_found`.
    pub async fn delete_message(&self, channel: &str, ts: &str) -> Result<()> {
        let params = vec![
            ("channel".to_string(), channel.to_string()),
            ("ts".to_string(), ts.to_string()),
        ];
        match self.api_call("chat.delete", params).await {
            Ok(_) => Ok(()),
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "cant_delete_message" =>
            {
                Err(SlackersError::Other(
                    "Cannot delete message: you can only delete your own messages unless you have admin permissions".to_string(),
                ))
            }
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "message_not_found" =>
            {
                Err(SlackersError::Other(format!(
                    "Message with ts '{}' not found in channel '{}'",
                    ts, channel
                )))
            }
            Err(e) => Err(e),
        }
    }

    // =========================================================================
    // Task 8.3: chat.update
    // =========================================================================

    /// Update the text of an existing message.
    ///
    /// Note: bots can only update their own messages.
    ///
    /// Returns descriptive errors for `cant_update_message` and
    /// `message_not_found`.
    pub async fn update_message(
        &self,
        channel: &str,
        ts: &str,
        text: &str,
    ) -> Result<ChatUpdateResponse> {
        let params = vec![
            ("channel".to_string(), channel.to_string()),
            ("ts".to_string(), ts.to_string()),
            ("text".to_string(), text.to_string()),
        ];
        let body = match self.api_call("chat.update", params).await {
            Ok(b) => b,
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "cant_update_message" =>
            {
                return Err(SlackersError::Other(
                    "Cannot update message: you can only update your own messages".to_string(),
                ));
            }
            Err(SlackersError::SlackApi { error_code: Some(ref code), .. })
                if code == "message_not_found" =>
            {
                return Err(SlackersError::Other(format!(
                    "Message with ts '{}' not found in channel '{}'",
                    ts, channel
                )));
            }
            Err(e) => return Err(e),
        };

        let response = ChatUpdateResponse {
            channel: body
                .get("channel")
                .and_then(|v| v.as_str())
                .unwrap_or(channel)
                .to_string(),
            ts: body
                .get("ts")
                .and_then(|v| v.as_str())
                .unwrap_or(ts)
                .to_string(),
            text: body
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or(text)
                .to_string(),
        };

        Ok(response)
    }

    // =========================================================================
    // Task 9.1: team.info / emoji.list
    // =========================================================================

    /// Fetch workspace (team) information via `team.info`.
    ///
    /// Returns a [`WorkspaceInfo`] struct with the workspace id, name, domain,
    /// and icon URLs.
    pub async fn get_workspace_info(&self) -> Result<WorkspaceInfo> {
        let body = self.api_call("team.info", vec![]).await?;

        let team = body
            .get("team")
            .ok_or_else(|| SlackersError::Other("No 'team' field in team.info response".to_string()))?;

        let info: WorkspaceInfo = serde_json::from_value(team.clone())
            .map_err(|e| SlackersError::Other(format!("Failed to parse WorkspaceInfo: {}", e)))?;

        Ok(info)
    }

    /// List all custom emoji for the workspace via `emoji.list`.
    ///
    /// Returns a map of emoji name → URL (or an alias string such as
    /// `"alias:other_emoji"`).  Aliases are included as-is.
    pub async fn list_emojis(&self) -> Result<HashMap<String, String>> {
        let body = self.api_call("emoji.list", vec![]).await?;

        let emoji_value = body
            .get("emoji")
            .ok_or_else(|| SlackersError::Other("No 'emoji' field in emoji.list response".to_string()))?;

        let emoji_map: HashMap<String, String> = serde_json::from_value(emoji_value.clone())
            .map_err(|e| SlackersError::Other(format!("Failed to parse emoji map: {}", e)))?;

        Ok(emoji_map)
    }

    // =========================================================================
    // channel mark: conversations.mark
    // =========================================================================

    /// Mark a channel or DM as read up to the given message timestamp.
    ///
    /// Calls `conversations.mark` with the provided channel ID and ts.
    pub async fn mark_channel(&self, channel_id: &str, ts: &str) -> Result<()> {
        let params = vec![
            ("channel".to_string(), channel_id.to_string()),
            ("ts".to_string(), ts.to_string()),
        ];
        self.api_call("conversations.mark", params).await?;
        Ok(())
    }

    /// Check if the API response indicates success
    fn check_api_response(body: Value) -> Result<Value> {
        if let Some(ok) = body.get("ok").and_then(|v| v.as_bool()) {
            if ok {
                return Ok(body);
            }
        }

        // Extract error message
        let error_msg = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error")
            .to_string();

        Err(SlackersError::from_slack_api(error_msg.clone(), Some(error_msg)))
    }
}

/// Detect a MIME type from a filename extension.
///
/// Falls back to `application/octet-stream` for unknown types.
fn detect_mime_type(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "txt" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "rs" | "py" | "go" | "java" | "c" | "cpp" | "h" | "sh" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn is_rate_limit_error(e: &SlackersError) -> bool {
    matches!(
        e,
        SlackersError::SlackApi {
            error_code: Some(code),
            ..
        } if code == "rate_limited" || code == "ratelimited"
    )
}

fn extract_retry_delay(e: &SlackersError, attempt: u32) -> u64 {
    // Exponential backoff: 5, 10, 20, 40, 60, 60, ... seconds
    let _ = e;
    let delay = BASE_RETRY_DELAY_SECS * (1u64 << (attempt - 1).min(3));
    delay.min(MAX_RETRY_DELAY_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let auth = WorkspaceAuth::Standard {
            token: "xoxb-test".to_string(),
        };
        let client = SlackClient::new(auth, None);
        assert!(client.workspace_url.is_none());
    }

    #[test]
    fn test_is_rate_limit_error() {
        let err = SlackersError::from_slack_api("rate limited", Some("rate_limited".to_string()));
        assert!(is_rate_limit_error(&err));

        let err = SlackersError::from_slack_api("invalid auth", Some("invalid_auth".to_string()));
        assert!(!is_rate_limit_error(&err));
    }

    // --- detect_mime_type tests ---

    #[test]
    fn test_detect_mime_type_common() {
        assert_eq!(detect_mime_type("report.pdf"), "application/pdf");
        assert_eq!(detect_mime_type("image.png"), "image/png");
        assert_eq!(detect_mime_type("photo.jpg"), "image/jpeg");
        assert_eq!(detect_mime_type("photo.JPEG"), "image/jpeg");
        assert_eq!(detect_mime_type("data.json"), "application/json");
        assert_eq!(detect_mime_type("style.css"), "text/css");
        assert_eq!(detect_mime_type("page.html"), "text/html");
        assert_eq!(detect_mime_type("archive.zip"), "application/zip");
    }

    #[test]
    fn test_detect_mime_type_source_files() {
        assert_eq!(detect_mime_type("main.rs"), "text/plain");
        assert_eq!(detect_mime_type("script.py"), "text/plain");
        assert_eq!(detect_mime_type("server.go"), "text/plain");
    }

    #[test]
    fn test_detect_mime_type_unknown() {
        assert_eq!(detect_mime_type("file.xyz"), "application/octet-stream");
        assert_eq!(detect_mime_type("noextension"), "application/octet-stream");
    }

    // --- FileUploadResponse serialization round-trip ---

    #[test]
    fn test_file_upload_response_serde() {
        let resp = FileUploadResponse {
            id: "F123ABC".to_string(),
            name: "report.pdf".to_string(),
            title: "Q1 Report".to_string(),
            permalink: Some("https://slack.com/files/...".to_string()),
            url_private: Some("https://files.slack.com/files-pri/...".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: FileUploadResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "F123ABC");
        assert_eq!(decoded.name, "report.pdf");
        assert_eq!(decoded.title, "Q1 Report");
        assert!(decoded.permalink.is_some());
        assert!(decoded.url_private.is_some());
    }

    // --- ChatUpdateResponse serialization round-trip ---

    #[test]
    fn test_chat_update_response_serde() {
        let resp = ChatUpdateResponse {
            channel: "C123".to_string(),
            ts: "1234567890.123456".to_string(),
            text: "Updated text".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: ChatUpdateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.channel, "C123");
        assert_eq!(decoded.ts, "1234567890.123456");
        assert_eq!(decoded.text, "Updated text");
    }

    // --- WorkspaceInfo deserialization ---

    #[test]
    fn test_workspace_info_deserialize() {
        let json = serde_json::json!({
            "id": "T12345",
            "name": "Acme Corp",
            "domain": "acme",
            "icon": {
                "image_34": "https://slack.com/icon_34.png",
                "image_44": "https://slack.com/icon_44.png",
                "image_68": null,
                "image_88": null,
                "image_102": null,
                "image_132": null,
                "image_230": null,
                "image_default": true
            }
        });
        let info: WorkspaceInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.id, "T12345");
        assert_eq!(info.name, "Acme Corp");
        assert_eq!(info.domain, "acme");
        let icon = info.icon.unwrap();
        assert_eq!(icon.image_34.as_deref(), Some("https://slack.com/icon_34.png"));
        assert_eq!(icon.image_default, Some(true));
    }

    #[test]
    fn test_workspace_info_no_icon() {
        let json = serde_json::json!({
            "id": "T99999",
            "name": "Test WS",
            "domain": "testws"
        });
        let info: WorkspaceInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.id, "T99999");
        assert!(info.icon.is_none());
    }

    // --- emoji map deserialization ---

    #[test]
    fn test_emoji_map_deserialize() {
        let json = serde_json::json!({
            "party_parrot": "https://emoji.slack-edge.com/party_parrot.gif",
            "wave": "alias:wave_anim"
        });
        let map: HashMap<String, String> = serde_json::from_value(json).unwrap();
        assert_eq!(
            map.get("party_parrot").map(|s| s.as_str()),
            Some("https://emoji.slack-edge.com/party_parrot.gif")
        );
        assert_eq!(map.get("wave").map(|s| s.as_str()), Some("alias:wave_anim"));
    }

    // --- ConversationOpenResponse serialization round-trip ---

    #[test]
    fn test_conversation_open_response_serde() {
        let channel_json: Value = serde_json::json!({
            "id": "D123",
            "is_im": true,
        });
        let resp = ConversationOpenResponse {
            id: "D123".to_string(),
            is_open: Some(true),
            already_open: Some(false),
            channel: channel_json,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: ConversationOpenResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "D123");
        assert_eq!(decoded.is_open, Some(true));
        assert_eq!(decoded.already_open, Some(false));
    }
}
