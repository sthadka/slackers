use crate::error::{ParseError, Result};
use regex::Regex;
use url::Url;

#[derive(Debug, Clone)]
pub struct SlackMessageRef {
    pub workspace_url: String,
    pub channel_id: String,
    pub message_ts: String,
    pub thread_ts_hint: Option<String>,
    #[allow(dead_code)]
    pub raw: String,
    #[allow(dead_code)]
    pub possibly_truncated: bool,
}

#[derive(Debug, Clone)]
pub enum MsgTarget {
    Url(SlackMessageRef),
    Channel(String),
}

/// Parse a message target (URL or channel identifier)
pub fn parse_msg_target(input: &str) -> Result<MsgTarget> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::MalformedTarget("Missing target".to_string()).into());
    }

    // Try parsing as URL first
    if let Ok(msg_ref) = parse_slack_message_url(trimmed) {
        return Ok(MsgTarget::Url(msg_ref));
    }

    // If starts with #, it's a channel name
    if trimmed.starts_with('#') {
        return Ok(MsgTarget::Channel(trimmed.to_string()));
    }

    // If looks like a channel ID, use it
    if is_channel_id(trimmed) {
        return Ok(MsgTarget::Channel(trimmed.to_string()));
    }

    // Allow bare channel names for convenience
    Ok(MsgTarget::Channel(format!("#{}", trimmed)))
}

/// Parse a Slack message URL
///
/// Format: https://*.slack.com/archives/<channel_id>/p<digits>?thread_ts=...
pub fn parse_slack_message_url(input: &str) -> Result<SlackMessageRef> {
    let url = Url::parse(input)
        .map_err(|e| ParseError::InvalidUrl(format!("{}: {}", input, e)))?;

    // Verify it's a Slack URL
    let host = url
        .host_str()
        .ok_or_else(|| ParseError::InvalidUrl(format!("No host in URL: {}", input)))?;

    if !host.ends_with(".slack.com") {
        return Err(ParseError::InvalidUrl(format!("Not a Slack workspace URL: {}", host)).into());
    }

    // Parse path: /archives/<channel>/<message>
    let path_segments: Vec<&str> = url
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if path_segments.len() < 3 || path_segments[0] != "archives" {
        return Err(
            ParseError::InvalidUrl(format!("Unsupported Slack URL path: {}", url.path())).into(),
        );
    }

    let channel_id = path_segments[1].to_string();
    let message_part = path_segments[2];

    // Parse message timestamp from p<digits> format
    let re = Regex::new(r"^p(\d{7,})$").unwrap();
    let captures = re
        .captures(message_part)
        .ok_or_else(|| ParseError::InvalidUrl(format!("Invalid message ID: {}", message_part)))?;

    let digits = captures.get(1).unwrap().as_str();
    if digits.len() <= 6 {
        return Err(ParseError::InvalidTimestamp(format!(
            "Message ID too short: {}",
            message_part
        ))
        .into());
    }

    // Split at position len-6 to get seconds.micros
    let split_pos = digits.len() - 6;
    let seconds = &digits[..split_pos];
    let micros = &digits[split_pos..];
    let message_ts = format!("{}.{}", seconds, micros);

    // Extract thread_ts hint from query params
    let thread_ts_param = url
        .query_pairs()
        .find(|(k, _)| k == "thread_ts")
        .map(|(_, v)| v.to_string());

    let thread_ts_hint = if let Some(ts) = thread_ts_param.as_ref() {
        // Validate thread_ts format
        let ts_re = Regex::new(r"^\d{6,}\.\d{6}$").unwrap();
        if ts_re.is_match(ts) {
            Some(ts.clone())
        } else {
            None
        }
    } else {
        None
    };

    // Detect possible truncation: has thread_ts but no cid param
    let has_cid = url.query_pairs().any(|(k, _)| k == "cid");
    let possibly_truncated = thread_ts_param.is_some() && !has_cid;

    let workspace_url = format!("{}://{}", url.scheme(), host);

    Ok(SlackMessageRef {
        workspace_url,
        channel_id,
        message_ts,
        thread_ts_hint,
        raw: input.to_string(),
        possibly_truncated,
    })
}

/// Check if a string looks like a Slack channel ID
///
/// Channel IDs match: ^[CDG][A-Z0-9]{8,}$
pub fn is_channel_id(input: &str) -> bool {
    let re = Regex::new(r"^[CDG][A-Z0-9]{8,}$").unwrap();
    re.is_match(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slack_message_url() {
        let url = "https://myteam.slack.com/archives/C0123456789/p1234567890123456";
        let result = parse_slack_message_url(url).unwrap();

        assert_eq!(result.workspace_url, "https://myteam.slack.com");
        assert_eq!(result.channel_id, "C0123456789");
        assert_eq!(result.message_ts, "1234567890.123456");
        assert_eq!(result.thread_ts_hint, None);
        assert!(!result.possibly_truncated);
    }

    #[test]
    fn test_parse_url_with_thread() {
        let url =
            "https://myteam.slack.com/archives/C0123456789/p1234567890123456?thread_ts=1234567890.123456&cid=C0123456789";
        let result = parse_slack_message_url(url).unwrap();

        assert_eq!(result.thread_ts_hint, Some("1234567890.123456".to_string()));
        assert!(!result.possibly_truncated);
    }

    #[test]
    fn test_parse_url_truncated() {
        let url =
            "https://myteam.slack.com/archives/C0123456789/p1234567890123456?thread_ts=1234567890.123456";
        let result = parse_slack_message_url(url).unwrap();

        assert!(result.possibly_truncated);
    }

    #[test]
    fn test_is_channel_id() {
        assert!(is_channel_id("C0123456789"));
        assert!(is_channel_id("D0123456789"));
        assert!(is_channel_id("G0123456789"));
        assert!(!is_channel_id("general"));
        assert!(!is_channel_id("#general"));
        assert!(!is_channel_id("X0123456789"));
    }

    #[test]
    fn test_parse_msg_target_url() {
        let target = "https://myteam.slack.com/archives/C0123456789/p1234567890123456";
        let result = parse_msg_target(target).unwrap();

        match result {
            MsgTarget::Url(ref_) => {
                assert_eq!(ref_.channel_id, "C0123456789");
            }
            _ => panic!("Expected URL target"),
        }
    }

    #[test]
    fn test_parse_msg_target_channel_name() {
        let result = parse_msg_target("#general").unwrap();
        match result {
            MsgTarget::Channel(ch) => assert_eq!(ch, "#general"),
            _ => panic!("Expected channel target"),
        }

        let result = parse_msg_target("general").unwrap();
        match result {
            MsgTarget::Channel(ch) => assert_eq!(ch, "#general"),
            _ => panic!("Expected channel target"),
        }
    }

    #[test]
    fn test_parse_msg_target_channel_id() {
        let result = parse_msg_target("C0123456789").unwrap();
        match result {
            MsgTarget::Channel(ch) => assert_eq!(ch, "C0123456789"),
            _ => panic!("Expected channel target"),
        }
    }
}
