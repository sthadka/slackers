use crate::error::Result;
use crate::slack::SlackClient;
use serde_json::Value;

/// Search for messages using Slack's search.messages API
///
/// Returns raw message matches from the search API with pagination support.
pub async fn search_messages_raw(
    client: &SlackClient,
    query: &str,
    limit: usize,
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
            ("sort".to_string(), "timestamp".to_string()),
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

/// Search for files using Slack's search.files API
///
/// Returns raw file matches from the search API with pagination support.
pub async fn search_files_raw(
    client: &SlackClient,
    query: &str,
    limit: usize,
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
            ("sort".to_string(), "timestamp".to_string()),
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
}
