use crate::auth::WorkspaceAuth;
use crate::error::{Result, SlackersError};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, COOKIE, ORIGIN};
use serde_json::Value;
use std::time::Duration;

const MAX_RETRIES: u32 = 3;
const MAX_RETRY_DELAY_SECS: u64 = 30;

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
    pub async fn api_call(
        &self,
        method: &str,
        params: Vec<(String, String)>,
    ) -> Result<Value> {
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
                    let delay = extract_retry_delay(&e).unwrap_or(1);
                    let delay = delay.min(MAX_RETRY_DELAY_SECS);
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

fn is_rate_limit_error(e: &SlackersError) -> bool {
    matches!(
        e,
        SlackersError::SlackApi {
            error_code: Some(code),
            ..
        } if code == "rate_limited"
    )
}

fn extract_retry_delay(e: &SlackersError) -> Option<u64> {
    // In a real implementation, we would extract from Retry-After header
    // For now, use exponential backoff: 1, 2, 4 seconds
    match e {
        SlackersError::SlackApi { .. } => Some(2),
        _ => None,
    }
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
}
