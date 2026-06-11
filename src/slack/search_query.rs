use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use regex::Regex;

/// Advanced query modifiers that map to Slack search operators
///
/// Each field that is `true` appends the corresponding operator to the query.
/// These operators work for both `search.messages` and `search.files`.
#[derive(Debug, Clone, Default)]
pub struct AdvancedQueryFilters {
    /// Append `has:link` — only results containing a URL
    pub has_link: bool,
    /// Append `has:emoji` — only results containing a custom emoji reaction
    pub has_emoji: bool,
    /// Append `from:me` — only results sent by the authenticated user
    pub from_me: bool,
}

impl AdvancedQueryFilters {
    /// Return the list of Slack query modifier tokens for enabled filters.
    ///
    /// The returned tokens are already correctly formatted and should be
    /// appended (space-separated) to the base query string.
    pub fn to_tokens(&self) -> Vec<String> {
        let mut tokens = Vec::new();
        if self.has_link {
            tokens.push("has:link".to_string());
        }
        if self.has_emoji {
            tokens.push("has:emoji".to_string());
        }
        if self.from_me {
            tokens.push("from:me".to_string());
        }
        tokens
    }

    /// Return `true` if no filters are active.
    pub fn is_empty(&self) -> bool {
        !self.has_link && !self.has_emoji && !self.from_me
    }
}

/// Build a Slack search query from components
///
/// Combines base query with modifiers like after:, before:, from:, in:
/// and optional advanced filters (has:link, has:emoji, from:me).
pub async fn build_search_query(
    client: &SlackClient,
    query: &str,
    channels: &[String],
    user: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
) -> Result<String> {
    build_search_query_with_filters(client, query, channels, user, after, before, None).await
}

/// Build a Slack search query with optional advanced filters.
///
/// This is the full-featured variant used internally when advanced filters
/// (has:link, has:emoji, from:me) are needed.  The simpler `build_search_query`
/// delegates to this function with `filters = None`.
pub async fn build_search_query_with_filters(
    client: &SlackClient,
    query: &str,
    channels: &[String],
    user: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    filters: Option<&AdvancedQueryFilters>,
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

    // Add advanced modifier tokens
    if let Some(f) = filters {
        parts.extend(f.to_tokens());
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

    #[test]
    fn test_advanced_query_filters_empty() {
        let f = AdvancedQueryFilters::default();
        assert!(f.is_empty());
        assert!(f.to_tokens().is_empty());
    }

    #[test]
    fn test_advanced_query_filters_has_link() {
        let f = AdvancedQueryFilters {
            has_link: true,
            ..Default::default()
        };
        assert!(!f.is_empty());
        assert_eq!(f.to_tokens(), vec!["has:link"]);
    }

    #[test]
    fn test_advanced_query_filters_has_emoji() {
        let f = AdvancedQueryFilters {
            has_emoji: true,
            ..Default::default()
        };
        assert_eq!(f.to_tokens(), vec!["has:emoji"]);
    }

    #[test]
    fn test_advanced_query_filters_from_me() {
        let f = AdvancedQueryFilters {
            from_me: true,
            ..Default::default()
        };
        assert_eq!(f.to_tokens(), vec!["from:me"]);
    }

    #[test]
    fn test_advanced_query_filters_combined() {
        let f = AdvancedQueryFilters {
            has_link: true,
            has_emoji: true,
            from_me: true,
        };
        assert!(!f.is_empty());
        let tokens = f.to_tokens();
        assert_eq!(tokens.len(), 3);
        assert!(tokens.contains(&"has:link".to_string()));
        assert!(tokens.contains(&"has:emoji".to_string()));
        assert!(tokens.contains(&"from:me".to_string()));
    }
}
