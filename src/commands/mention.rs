use crate::auth::resolve_auth;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::{self, MentionsOptions, SlackClient, SortOrder};

/// List messages that @mention the authenticated user or a named user.
///
/// Outputs a JSON array of compact messages.
pub async fn handle_mention_list(
    workspace: Option<&str>,
    username: Option<String>,
    channels: Vec<String>,
    after: Option<String>,
    before: Option<String>,
    limit: usize,
    max_body_chars: i32,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url.clone());

    let max_content_chars = if max_body_chars < 0 {
        usize::MAX
    } else {
        max_body_chars as usize
    };

    let options = MentionsOptions {
        workspace_url: auth_result.workspace_url.as_deref(),
        username: username.as_deref(),
        channels: &channels,
        after: after.as_deref(),
        before: before.as_deref(),
        limit,
        max_content_chars,
        download: false,
        sort: SortOrder::Timestamp,
    };

    let messages = slack::search_mentions(&client, &auth_result.auth, options).await?;

    println!("{}", to_json_output(&messages));
    Ok(())
}
