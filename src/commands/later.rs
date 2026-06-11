use crate::auth::resolve_auth;
use crate::cli::{LaterAddOptions, LaterListOptions, LaterRemoveOptions};
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::SlackClient;
use serde_json::json;

/// Add a message to stars (save for later).
pub async fn handle_later_add(opts: LaterAddOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    client.star_message(&opts.channel, &opts.ts).await?;

    let output = json!({ "ok": true });
    println!("{}", to_json_output(&output));

    Ok(())
}

/// Remove a message from stars (unsave).
pub async fn handle_later_remove(opts: LaterRemoveOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    client.unstar_message(&opts.channel, &opts.ts).await?;

    let output = json!({ "ok": true });
    println!("{}", to_json_output(&output));

    Ok(())
}

/// List starred items (saved for later).
pub async fn handle_later_list(opts: LaterListOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let items = client.list_starred(opts.limit).await?;

    let output = json!(items);
    println!("{}", to_json_output(&output));

    Ok(())
}
