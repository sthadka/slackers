use crate::auth::WorkspaceAuth;
use crate::error::{Result, SlackersError};
use crate::render::html_to_markdown;
use crate::slack::SlackClient;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE};

/// Parse a canvas identifier (URL or file ID)
///
/// Accepts:
/// - Full URL: https://*.slack.com/docs/T.../F...
/// - Bare file ID: F...
pub fn parse_canvas_identifier(input: &str) -> Result<String> {
    let trimmed = input.trim();

    // If it starts with F, it's already a file ID
    if trimmed.starts_with('F') {
        return Ok(trimmed.to_string());
    }

    // Try to parse as URL
    let url_re = Regex::new(r"https://[^/]+\.slack\.com/docs/[^/]+/(F[A-Z0-9]+)").unwrap();
    if let Some(captures) = url_re.captures(trimmed) {
        if let Some(file_id) = captures.get(1) {
            return Ok(file_id.as_str().to_string());
        }
    }

    Err(SlackersError::Other(format!(
        "Invalid canvas identifier: {}. Expected URL or file ID starting with F",
        input
    )))
}

/// Fetch a canvas and convert to Markdown
///
/// Returns the Markdown content, truncated to max_chars if specified.
pub async fn fetch_canvas(
    client: &SlackClient,
    auth: &WorkspaceAuth,
    file_id: &str,
    max_chars: Option<usize>,
) -> Result<String> {
    // Get file info to find download URL
    let params = vec![("file".to_string(), file_id.to_string())];
    let response = client.api_call("files.info", params).await?;

    let file = response
        .get("file")
        .ok_or_else(|| SlackersError::Other("No file in response".to_string()))?;

    // Get download URL
    let download_url = file
        .get("url_private")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SlackersError::Other("No download URL for canvas".to_string()))?;

    // Download HTML content
    let html = download_canvas_html(download_url, auth).await?;

    // Convert to Markdown
    let mut markdown = html_to_markdown(&html);

    // Truncate if needed
    if let Some(max) = max_chars {
        if markdown.len() > max {
            markdown.truncate(max);
            markdown.push_str("\n\n...[truncated]");
        }
    }

    Ok(markdown)
}

/// Download canvas HTML content with authentication
async fn download_canvas_html(url: &str, auth: &WorkspaceAuth) -> Result<String> {
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
        }
    }

    let http_client = reqwest::Client::new();
    let response = http_client.get(url).headers(headers).send().await?;

    if !response.status().is_success() {
        return Err(SlackersError::Other(format!(
            "Failed to download canvas: HTTP {}",
            response.status()
        )));
    }

    let html = response.text().await?;
    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_canvas_identifier_bare_id() {
        let result = parse_canvas_identifier("F0123456789").unwrap();
        assert_eq!(result, "F0123456789");
    }

    #[test]
    fn test_parse_canvas_identifier_url() {
        let url = "https://myteam.slack.com/docs/T0123456789/F9876543210";
        let result = parse_canvas_identifier(url).unwrap();
        assert_eq!(result, "F9876543210");
    }

    #[test]
    fn test_parse_canvas_identifier_url_with_hash() {
        let url = "https://example.slack.com/docs/T123/F456#heading";
        let result = parse_canvas_identifier(url).unwrap();
        assert_eq!(result, "F456");
    }

    #[test]
    fn test_parse_canvas_identifier_invalid() {
        let result = parse_canvas_identifier("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_canvas_identifier_invalid_url() {
        let result = parse_canvas_identifier("https://example.com/other");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_canvas_identifier_whitespace() {
        let result = parse_canvas_identifier("  F0123456789  ").unwrap();
        assert_eq!(result, "F0123456789");
    }
}
