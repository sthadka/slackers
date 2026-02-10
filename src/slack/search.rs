use crate::auth::WorkspaceAuth;
use crate::error::Result;
use crate::slack::{
    build_search_query, search_files, search_messages, CompactSlackMessage, ContentType,
    FileSearchResult, SearchFilesInput, SearchMessagesInput, SlackClient,
};

#[derive(Debug, Clone, PartialEq)]
pub enum SearchKind {
    Messages,
    Files,
    All,
}

pub struct SearchOptions<'a> {
    pub workspace_url: Option<&'a str>,
    pub query: &'a str,
    pub kind: SearchKind,
    pub channels: &'a [String],
    pub user: Option<&'a str>,
    pub after: Option<&'a str>,
    pub before: Option<&'a str>,
    pub content_type: ContentType,
    pub limit: usize,
    pub max_content_chars: usize,
    pub download: bool,
}

impl<'a> Default for SearchOptions<'a> {
    fn default() -> Self {
        SearchOptions {
            workspace_url: None,
            query: "",
            kind: SearchKind::All,
            channels: &[],
            user: None,
            after: None,
            before: None,
            content_type: ContentType::Any,
            limit: 20,
            max_content_chars: 4000,
            download: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct SearchResult {
    pub messages: Option<Vec<CompactSlackMessage>>,
    pub files: Option<Vec<FileSearchResult>>,
}

/// Orchestrate Slack search based on options
///
/// Routes to appropriate search modules based on kind (messages, files, or all).
/// Applies shared defaults: limit=20 (max 200), max_content_chars=4000.
pub async fn search_slack(
    client: &SlackClient,
    auth: &WorkspaceAuth,
    options: SearchOptions<'_>,
) -> Result<SearchResult> {
    // Apply limits
    let limit = options.limit.clamp(1, 200);
    let max_content_chars = options.max_content_chars;
    let content_type = options.content_type.clone();
    let download = options.download;

    // Build Slack search query
    let slack_query = build_search_query(
        client,
        options.query,
        options.channels,
        options.user,
        options.after,
        options.before,
    )
    .await?;

    let mut result = SearchResult::default();

    // Search messages if requested
    if options.kind == SearchKind::Messages || options.kind == SearchKind::All {
        let messages_input = SearchMessagesInput {
            auth,
            workspace_url: options.workspace_url,
            query: &slack_query,
            limit,
            max_content_chars,
            content_type: content_type.clone(),
            download,
        };

        let messages = search_messages(client, messages_input).await?;
        result.messages = Some(messages);
    }

    // Search files if requested
    if options.kind == SearchKind::Files || options.kind == SearchKind::All {
        let files_input = SearchFilesInput {
            auth,
            query: &slack_query,
            limit,
            content_type: content_type.clone(),
        };

        let files = search_files(client, files_input).await?;
        result.files = Some(files);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_options_default() {
        let opts = SearchOptions::default();
        assert_eq!(opts.limit, 20);
        assert_eq!(opts.max_content_chars, 4000);
        assert!(opts.download);
        assert_eq!(opts.content_type, ContentType::Any);
    }

    #[test]
    fn test_search_kind() {
        assert_eq!(SearchKind::Messages, SearchKind::Messages);
        assert_ne!(SearchKind::Messages, SearchKind::Files);
    }

    #[test]
    fn test_limit_clamping() {
        let limit = 0_usize.clamp(1, 200);
        assert_eq!(limit, 1);

        let limit = 300_usize.clamp(1, 200);
        assert_eq!(limit, 200);

        let limit = 50_usize.clamp(1, 200);
        assert_eq!(limit, 50);
    }
}
