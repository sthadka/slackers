use crate::auth::resolve_auth;
use crate::cli::SearchCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::{
    search_slack, AdvancedQueryFilters, ContentType, SearchKind, SearchOptions, SlackClient,
    SortOrder,
};
use serde_json::json;

pub async fn handle_search(subcommand: SearchCommand) -> Result<()> {
    match subcommand {
        SearchCommand::All { query, options } => {
            handle_search_impl(&query, SearchKind::All, options).await
        }
        SearchCommand::Messages { query, options } => {
            handle_search_impl(&query, SearchKind::Messages, options).await
        }
        SearchCommand::Files { query, options } => {
            handle_search_impl(&query, SearchKind::Files, options).await
        }
    }
}

async fn handle_search_impl(
    query: &str,
    kind: SearchKind,
    options: crate::cli::SearchOptions,
) -> Result<()> {
    // Resolve auth
    let auth_result = resolve_auth(options.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url.clone());

    // Parse content type
    let content_type = match options.content_type.as_deref() {
        Some("text") => ContentType::Text,
        Some("image") => ContentType::Image,
        Some("snippet") => ContentType::Snippet,
        Some("file") => ContentType::File,
        _ => ContentType::Any,
    };

    // Parse sort order
    let sort = match options.sort.as_deref() {
        Some("relevance") | Some("score") => SortOrder::Relevance,
        _ => SortOrder::Timestamp,
    };

    // Parse advanced filters
    let advanced_filters = AdvancedQueryFilters {
        has_link: options.has_link,
        has_emoji: options.has_emoji,
        from_me: options.from_me,
    };

    // Build search options
    let max_content_chars = if options.max_content_chars < 0 {
        usize::MAX
    } else {
        options.max_content_chars as usize
    };

    let search_opts = SearchOptions {
        workspace_url: auth_result.workspace_url.as_deref(),
        query,
        kind,
        channels: &options.channel,
        user: options.user.as_deref(),
        after: options.after.as_deref(),
        before: options.before.as_deref(),
        content_type,
        limit: options.limit as usize,
        max_content_chars,
        download: true, // Always download files for local access
        sort,
        advanced_filters,
    };

    // Execute search
    let result = search_slack(&client, &auth_result.auth, search_opts).await?;

    // Build output
    let output = json!({
        "messages": result.messages,
        "files": result.files,
    });

    println!("{}", to_json_output(&output));
    Ok(())
}
