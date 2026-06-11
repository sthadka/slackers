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

/// Options for searching mentions
///
/// When `username` is None the query uses `to:@me` which matches the
/// authenticated user's mentions (standard token only).  When a username is
/// supplied the query uses `mentions:@username` which works regardless of
/// whose token is in use.
pub struct MentionsOptions<'a> {
    /// Slack workspace URL (required for browser-auth clients)
    pub workspace_url: Option<&'a str>,
    /// Username to search mentions for. None → current user ("to:@me").
    pub username: Option<&'a str>,
    /// Narrow results to these channels (#name, name, or channel-id)
    pub channels: &'a [String],
    /// Only results after YYYY-MM-DD
    pub after: Option<&'a str>,
    /// Only results before YYYY-MM-DD
    pub before: Option<&'a str>,
    /// Max results (clamped 1–200)
    pub limit: usize,
    /// Max message content characters (use usize::MAX for unlimited)
    pub max_content_chars: usize,
    /// Whether to download attached files locally
    pub download: bool,
}

impl<'a> Default for MentionsOptions<'a> {
    fn default() -> Self {
        MentionsOptions {
            workspace_url: None,
            username: None,
            channels: &[],
            after: None,
            before: None,
            limit: 20,
            max_content_chars: 4000,
            download: false,
        }
    }
}

/// Build the base query token for mentions search.
///
/// Returns `"to:@me"` when `username` is `None` (self-mentions via standard
/// Slack token) or `"mentions:@<username>"` for a named user.
pub fn mentions_query_token(username: Option<&str>) -> String {
    match username {
        None => "to:@me".to_string(),
        Some(u) => {
            // Strip leading @ if caller included it
            let name = u.trim_start_matches('@');
            format!("mentions:@{}", name)
        }
    }
}

/// Search for messages that mention the authenticated user (or a named user).
///
/// Uses Slack's `search.messages` API with a `to:@me` (or `mentions:@name`)
/// base query, optionally filtered by channel and date range.  Pagination is
/// handled transparently up to `options.limit` results.
pub async fn search_mentions(
    client: &SlackClient,
    auth: &WorkspaceAuth,
    options: MentionsOptions<'_>,
) -> Result<Vec<CompactSlackMessage>> {
    let limit = options.limit.clamp(1, 200);

    // Build the base mention token
    let mention_token = mentions_query_token(options.username);

    // Let build_search_query add channel / date modifiers on top of our base
    // token.  We pass it as the `query` argument so it ends up at the front.
    let slack_query = build_search_query(
        client,
        &mention_token,
        options.channels,
        None, // user filter not applicable for mentions search
        options.after,
        options.before,
    )
    .await?;

    let messages_input = SearchMessagesInput {
        auth,
        workspace_url: options.workspace_url,
        query: &slack_query,
        limit,
        max_content_chars: options.max_content_chars,
        content_type: ContentType::Any,
        download: options.download,
    };

    search_messages(client, messages_input).await
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

    #[test]
    fn test_mentions_query_token_self() {
        assert_eq!(mentions_query_token(None), "to:@me");
    }

    #[test]
    fn test_mentions_query_token_named_user() {
        assert_eq!(mentions_query_token(Some("alice")), "mentions:@alice");
        assert_eq!(mentions_query_token(Some("@bob")), "mentions:@bob");
    }

    #[test]
    fn test_mentions_options_default() {
        let opts = MentionsOptions::default();
        assert_eq!(opts.limit, 20);
        assert_eq!(opts.max_content_chars, 4000);
        assert!(!opts.download);
        assert!(opts.username.is_none());
    }
}
