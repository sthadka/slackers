use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use regex::Regex;

/// Build a Slack search query from components
///
/// Combines base query with modifiers like after:, before:, from:, in:
pub async fn build_search_query(
    client: &SlackClient,
    query: &str,
    channels: &[String],
    user: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
) -> Result<String> {
    let mut parts = Vec::new();

    // Add base query
    let base = query.trim();
    if !base.is_empty() {
        parts.push(base.to_string());
    }

    // Add date filters
    if let Some(after_date) = after {
        let validated = validate_date(after_date)?;
        parts.push(format!("after:{}", validated));
    }

    if let Some(before_date) = before {
        let validated = validate_date(before_date)?;
        parts.push(format!("before:{}", validated));
    }

    // Add user filter
    if let Some(user_input) = user {
        if let Some(token) = user_token_for_search(client, user_input).await? {
            parts.push(token);
        }
    }

    // Add channel filters
    for channel in channels {
        if let Some(token) = channel_token_for_search(client, channel).await? {
            parts.push(token);
        }
    }

    Ok(parts.join(" "))
}

/// Validate date format (YYYY-MM-DD)
pub fn validate_date(date: &str) -> Result<String> {
    let trimmed = date.trim();
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();

    if !re.is_match(trimmed) {
        return Err(SlackersError::Other(format!(
            "Invalid date: {} (expected YYYY-MM-DD)",
            date
        )));
    }

    Ok(trimmed.to_string())
}

/// Convert user input to search token (from:@name)
async fn user_token_for_search(client: &SlackClient, user: &str) -> Result<Option<String>> {
    let trimmed = user.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // If starts with @, use as-is
    if trimmed.starts_with('@') {
        return Ok(Some(format!("from:@{}", &trimmed[1..])));
    }

    // If it's a user ID (U...), resolve to name
    let user_id_re = Regex::new(r"^U[A-Z0-9]{8,}$").unwrap();
    if user_id_re.is_match(trimmed) {
        match resolve_user_name(client, trimmed).await {
            Ok(Some(name)) => return Ok(Some(format!("from:@{}", name))),
            _ => return Ok(None),
        }
    }

    // Otherwise, treat as username
    Ok(Some(format!("from:@{}", trimmed)))
}

/// Resolve user ID to username
async fn resolve_user_name(client: &SlackClient, user_id: &str) -> Result<Option<String>> {
    let params = vec![("user".to_string(), user_id.to_string())];

    match client.api_call("users.info", params).await {
        Ok(response) => {
            let name = response
                .get("user")
                .and_then(|u| u.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
            Ok(name)
        }
        Err(_) => Ok(None),
    }
}

/// Convert channel input to search token (in:#name)
async fn channel_token_for_search(client: &SlackClient, channel: &str) -> Result<Option<String>> {
    let trimmed = channel.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // If starts with #, it's already a channel name
    if trimmed.starts_with('#') {
        return Ok(Some(format!("in:{}", trimmed)));
    }

    // If it looks like a channel ID, resolve to name
    let channel_id_re = Regex::new(r"^[CDG][A-Z0-9]{8,}$").unwrap();
    if channel_id_re.is_match(trimmed) {
        match resolve_channel_name(client, trimmed).await {
            Ok(Some(name)) => return Ok(Some(format!("in:#{}", name))),
            _ => return Ok(None),
        }
    }

    // Otherwise, treat as channel name (add # prefix)
    Ok(Some(format!("in:#{}", trimmed)))
}

/// Resolve channel ID to channel name
async fn resolve_channel_name(client: &SlackClient, channel_id: &str) -> Result<Option<String>> {
    let params = vec![("channel".to_string(), channel_id.to_string())];

    match client.api_call("conversations.info", params).await {
        Ok(response) => {
            let name = response
                .get("channel")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
            Ok(name)
        }
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_date_valid() {
        assert_eq!(validate_date("2024-01-15").unwrap(), "2024-01-15");
        assert_eq!(validate_date("  2024-12-31  ").unwrap(), "2024-12-31");
    }

    #[test]
    fn test_validate_date_invalid() {
        assert!(validate_date("2024-1-15").is_err());
        assert!(validate_date("2024/01/15").is_err());
        assert!(validate_date("01-15-2024").is_err());
        assert!(validate_date("not-a-date").is_err());
    }
}
