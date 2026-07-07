use crate::error::Result;
use crate::slack::SlackClient;
use serde_json::Value;

/// Sort order for Slack search results
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SortOrder {
    /// Sort by relevance score (Slack default for search)
    Relevance,
    /// Sort by message/file timestamp, newest first
    #[default]
    Timestamp,
}

impl SortOrder {
    /// Return the Slack API `sort` parameter value for this order.
    pub fn as_slack_param(&self) -> &'static str {
        match self {
            SortOrder::Relevance => "score",
            SortOrder::Timestamp => "timestamp",
        }
    }
}

/// Search for messages with explicit sort order.
///
/// This is the full-featured variant; `search_messages_raw` delegates here
/// with `SortOrder::Timestamp` to preserve backwards compatibility.
pub async fn search_messages_raw_sorted(
    client: &SlackClient,
    query: &str,
    limit: usize,
    sort: &SortOrder,
) -> Result<Vec<Value>> {
    let page_size = limit.min(100).max(1);
    let mut results = Vec::new();
    let mut page = 1;
    let mut total_pages = 1;

    loop {
        // Call search.messages API
        let params = vec![
            ("query".to_string(), query.to_string()),
            ("count".to_string(), page_size.to_string()),
            ("page".to_string(), page.to_string()),
            ("highlight".to_string(), "false".to_string()),
            ("sort".to_string(), sort.as_slack_param().to_string()),
            ("sort_dir".to_string(), "desc".to_string()),
        ];

        let response = client.api_call("search.messages", params).await?;

        // Extract matches from response.messages.matches
        let matches = response
            .get("messages")
            .and_then(|m| m.get("matches"))
            .and_then(|m| m.as_array())
            .map(|arr| arr.clone())
            .unwrap_or_default();

        let matches_count = matches.len();
        results.extend(matches);

        // Extract pagination info from response.messages.paging or response.messages.pagination
        if let Some(messages) = response.get("messages") {
            let paging = messages
                .get("paging")
                .or_else(|| messages.get("pagination"));

            if let Some(paging) = paging {
                if let Some(pages) = paging.get("pages").and_then(|p| p.as_u64()) {
                    total_pages = pages as usize;
                }
            }
        }

        // Check stopping conditions
        if results.len() >= limit {
            break;
        }
        if matches_count == 0 {
            break;
        }
        if page >= total_pages {
            break;
        }

        page += 1;
    }

    // Truncate to requested limit
    results.truncate(limit);
    Ok(results)
}

/// Search for files with explicit sort order.
///
/// This is the full-featured variant; `search_files_raw` delegates here
/// with `SortOrder::Timestamp` to preserve backwards compatibility.
pub async fn search_files_raw_sorted(
    client: &SlackClient,
    query: &str,
    limit: usize,
    sort: &SortOrder,
) -> Result<Vec<Value>> {
    let page_size = limit.min(100).max(1);
    let mut results = Vec::new();
    let mut page = 1;
    let mut total_pages = 1;

    loop {
        // Call search.files API
        let params = vec![
            ("query".to_string(), query.to_string()),
            ("count".to_string(), page_size.to_string()),
            ("page".to_string(), page.to_string()),
            ("highlight".to_string(), "false".to_string()),
            ("sort".to_string(), sort.as_slack_param().to_string()),
            ("sort_dir".to_string(), "desc".to_string()),
        ];

        let response = client.api_call("search.files", params).await?;

        // Extract matches from response.files.matches
        let matches = response
            .get("files")
            .and_then(|f| f.get("matches"))
            .and_then(|m| m.as_array())
            .map(|arr| arr.clone())
            .unwrap_or_default();

        let matches_count = matches.len();
        results.extend(matches);

        // Extract pagination info from response.files.paging or response.files.pagination
        if let Some(files) = response.get("files") {
            let paging = files.get("paging").or_else(|| files.get("pagination"));

            if let Some(paging) = paging {
                if let Some(pages) = paging.get("pages").and_then(|p| p.as_u64()) {
                    total_pages = pages as usize;
                }
            }
        }

        // Check stopping conditions
        if results.len() >= limit {
            break;
        }
        if matches_count == 0 {
            break;
        }
        if page >= total_pages {
            break;
        }

        page += 1;
    }

    // Truncate to requested limit
    results.truncate(limit);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_size_calculation() {
        // Page size should be clamped between 1 and 100
        let limit = 0;
        let page_size = limit.min(100).max(1);
        assert_eq!(page_size, 1);

        let limit = 150;
        let page_size = limit.min(100).max(1);
        assert_eq!(page_size, 100);

        let limit = 50;
        let page_size = limit.min(100).max(1);
        assert_eq!(page_size, 50);
    }

    #[test]
    fn test_sort_order_slack_param() {
        assert_eq!(SortOrder::Relevance.as_slack_param(), "score");
        assert_eq!(SortOrder::Timestamp.as_slack_param(), "timestamp");
    }

    #[test]
    fn test_sort_order_default() {
        assert_eq!(SortOrder::default(), SortOrder::Timestamp);
    }
}
