use crate::auth::WorkspaceAuth;
use crate::error::Result;
use crate::slack::{download_file, search_files_raw, ContentType, SlackClient};
use std::path::PathBuf;

pub struct SearchFilesInput<'a> {
    pub auth: &'a WorkspaceAuth,
    pub query: &'a str,
    pub limit: usize,
    pub content_type: ContentType,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileSearchResult {
    pub title: Option<String>,
    pub mimetype: Option<String>,
    pub mode: Option<String>,
    pub path: PathBuf,
}

/// Search for files using the search.files API
///
/// Downloads matching files and returns their local paths.
pub async fn search_files(
    client: &SlackClient,
    input: SearchFilesInput<'_>,
) -> Result<Vec<FileSearchResult>> {
    // Get raw matches from search API
    let raw_matches = search_files_raw(client, input.query, input.limit).await?;

    if raw_matches.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    for file_match in &raw_matches {
        // Extract file metadata
        let mode = file_match
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mimetype = file_match
            .get("mimetype")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Apply content type filter
        if !passes_file_content_type_filter(mode.as_deref(), mimetype.as_deref(), &input.content_type) {
            continue;
        }

        // Try to download the file
        if let Ok(path) = download_file(client, file_match, input.auth).await {
            let title = file_match
                .get("title")
                .and_then(|v| v.as_str())
                .or_else(|| file_match.get("name").and_then(|v| v.as_str()))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            results.push(FileSearchResult {
                title,
                mimetype,
                mode,
                path,
            });

            if results.len() >= input.limit {
                break;
            }
        }
    }

    Ok(results)
}

/// Check if file passes content type filter
fn passes_file_content_type_filter(
    mode: Option<&str>,
    mimetype: Option<&str>,
    content_type: &ContentType,
) -> bool {
    match content_type {
        ContentType::Any | ContentType::File => true,
        ContentType::Snippet => mode == Some("snippet"),
        ContentType::Image => mimetype
            .map(|m| m.to_lowercase().starts_with("image/"))
            .unwrap_or(false),
        ContentType::Text => mimetype == Some("text/plain"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_content_type_filter_any() {
        assert!(passes_file_content_type_filter(
            None,
            None,
            &ContentType::Any
        ));
        assert!(passes_file_content_type_filter(
            Some("snippet"),
            Some("text/plain"),
            &ContentType::Any
        ));
    }

    #[test]
    fn test_file_content_type_filter_snippet() {
        assert!(passes_file_content_type_filter(
            Some("snippet"),
            None,
            &ContentType::Snippet
        ));
        assert!(!passes_file_content_type_filter(
            Some("hosted"),
            None,
            &ContentType::Snippet
        ));
        assert!(!passes_file_content_type_filter(
            None,
            None,
            &ContentType::Snippet
        ));
    }

    #[test]
    fn test_file_content_type_filter_image() {
        assert!(passes_file_content_type_filter(
            None,
            Some("image/png"),
            &ContentType::Image
        ));
        assert!(passes_file_content_type_filter(
            None,
            Some("image/jpeg"),
            &ContentType::Image
        ));
        assert!(!passes_file_content_type_filter(
            None,
            Some("text/plain"),
            &ContentType::Image
        ));
        assert!(!passes_file_content_type_filter(
            None,
            None,
            &ContentType::Image
        ));
    }

    #[test]
    fn test_file_content_type_filter_text() {
        assert!(passes_file_content_type_filter(
            None,
            Some("text/plain"),
            &ContentType::Text
        ));
        assert!(!passes_file_content_type_filter(
            None,
            Some("image/png"),
            &ContentType::Text
        ));
        assert!(!passes_file_content_type_filter(
            None,
            None,
            &ContentType::Text
        ));
    }
}
