use crate::auth::WorkspaceAuth;
use crate::config::{downloads_dir, ensure_dir};
use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, REFERER};
use serde_json::Value;
use std::path::PathBuf;

/// Download a Slack file to the local downloads directory
///
/// Returns the path to the downloaded file.
/// Skips download if file already exists (cached).
pub async fn download_file(
    _client: &SlackClient,
    file_info: &Value,
    auth: &WorkspaceAuth,
    workspace_url: Option<&str>,
) -> Result<PathBuf> {
    // Extract file information
    let file_id = file_info
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SlackersError::Other("Missing file ID".to_string()))?;

    let file_name = file_info
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SlackersError::Other("Missing file name".to_string()))?;

    // Get download URL (prefer url_private_download over url_private)
    let download_url = file_info
        .get("url_private_download")
        .and_then(|v| v.as_str())
        .or_else(|| file_info.get("url_private").and_then(|v| v.as_str()))
        .ok_or_else(|| SlackersError::Other("No download URL available".to_string()))?;

    // Sanitize filename
    let safe_filename = sanitize_filename(file_name);
    let filename_with_id = format!("{}_{}", file_id, safe_filename);

    // Determine download path
    let download_path = downloads_dir()?.join(&filename_with_id);

    // Skip if already downloaded
    if download_path.exists() {
        return Ok(download_path);
    }

    // Ensure downloads directory exists
    ensure_dir(&downloads_dir()?)?;

    // Build authenticated request
    let mut headers = HeaderMap::new();

    match auth {
        WorkspaceAuth::Standard { token } => {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token))
                    .map_err(|e| SlackersError::Other(e.to_string()))?,
            );
        }
        WorkspaceAuth::Browser {
            xoxc_token,
            xoxd_cookie,
        } => {
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
            let referer = workspace_url.unwrap_or("https://app.slack.com");
            headers.insert(
                REFERER,
                HeaderValue::from_str(referer)
                    .map_err(|e| SlackersError::Other(e.to_string()))?,
            );
        }
    }

    // Download file
    let http_client = reqwest::Client::new();
    let response = http_client
        .get(download_url)
        .headers(headers)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(SlackersError::Other(format!(
            "Failed to download file: HTTP {}",
            response.status()
        )));
    }

    // Check content type - reject HTML (indicates auth failure)
    if let Some(content_type) = response.headers().get("content-type") {
        if let Ok(ct_str) = content_type.to_str() {
            if ct_str.contains("text/html") {
                return Err(SlackersError::Other(
                    "Received HTML response - authentication may have failed".to_string(),
                ));
            }
        }
    }

    // Write to file
    let bytes = response.bytes().await?;
    std::fs::write(&download_path, bytes)
        .map_err(|e| SlackersError::Other(format!("Failed to write file: {}", e)))?;

    Ok(download_path)
}

/// Enrich file information by calling files.info API
///
/// Useful for snippets or files missing download URLs
#[allow(dead_code)]
pub async fn enrich_file_info(client: &SlackClient, file_id: &str) -> Result<Value> {
    let params = vec![("file".to_string(), file_id.to_string())];
    let response = client.api_call("files.info", params).await?;

    response
        .get("file")
        .cloned()
        .ok_or_else(|| SlackersError::Other("No file data in response".to_string()))
}

/// Sanitize a filename by removing/replacing special characters
fn sanitize_filename(filename: &str) -> String {
    // Replace problematic characters with underscores
    let re = Regex::new(r#"[/\\:*?"<>|]"#).unwrap();
    let replacement = "_";
    let sanitized = re.replace_all(filename, replacement).to_string();

    // Limit length to avoid filesystem issues
    let max_len = 200;
    if sanitized.len() > max_len {
        // Try to preserve file extension
        if let Some(dot_pos) = sanitized.rfind('.') {
            if sanitized.len() - dot_pos < 10 {
                // Extension is reasonable length
                let ext = &sanitized[dot_pos..];
                let name_len = max_len - ext.len();
                return format!("{}{}", &sanitized[..name_len], ext);
            }
        }
        sanitized[..max_len].to_string()
    } else {
        sanitized.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_simple() {
        let result1 = sanitize_filename("document.pdf");
        assert_eq!(result1, "document.pdf");

        let result2 = sanitize_filename("my file.txt");
        assert_eq!(result2, "my file.txt");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(sanitize_filename("file/name.pdf"), "file_name.pdf");
        assert_eq!(sanitize_filename(r"file\name.pdf"), "file_name.pdf");
        assert_eq!(sanitize_filename("file:name.pdf"), "file_name.pdf");
        assert_eq!(sanitize_filename("file*name?.pdf"), "file_name_.pdf");
        assert_eq!(sanitize_filename("file<name>.pdf"), "file_name_.pdf");
        assert_eq!(sanitize_filename("file|name.pdf"), "file_name.pdf");
    }

    #[test]
    fn test_sanitize_filename_long() {
        let mut long_name = String::from("a").repeat(250);
        long_name.push_str(".pdf");
        let result = sanitize_filename(&long_name);
        assert!(result.len() <= 200);
        assert!(result.ends_with(".pdf"));
    }

    #[test]
    fn test_sanitize_filename_long_without_extension() {
        let long_name = String::from("a").repeat(250);
        let result = sanitize_filename(&long_name);
        assert_eq!(result.len(), 200);
    }

    #[test]
    fn test_sanitize_filename_preserves_extension() {
        let mut name = String::from("a").repeat(250);
        name.push_str(".custom");
        let result = sanitize_filename(&name);
        assert!(result.ends_with(".custom"));
        assert!(result.len() <= 200);
    }
}
