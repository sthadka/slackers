use crate::auth::resolve_auth;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::{format_outbound_slack_text, SlackClient};
use serde_json::json;

/// Open a direct message conversation with one or more users.
///
/// Outputs JSON: `{"ok": true, "channel_id": "<id>"}`.
pub async fn handle_dm_open(
    workspace: Option<&str>,
    user_ids: Vec<String>,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let resp = client.open_conversation(user_ids).await?;

    let output = json!({
        "ok": true,
        "channel_id": resp.id,
    });

    println!("{}", to_json_output(&output));
    Ok(())
}

/// Open a DM with one or more users and send a message into it.
///
/// Outputs JSON: `{"ok": true, "channel_id": "<id>", "ts": "<ts>"}`.
pub async fn handle_dm_send(
    workspace: Option<&str>,
    user_ids: Vec<String>,
    message: String,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Step 1: open (or retrieve) the DM conversation
    let conv = client.open_conversation(user_ids).await?;
    let channel_id = conv.id;

    // Step 2: post a message to the DM channel
    let formatted_message = format_outbound_slack_text(&message);
    let params = vec![
        ("channel".to_string(), channel_id.clone()),
        ("text".to_string(), formatted_message),
    ];
    let response = client.api_call("chat.postMessage", params).await?;

    let ts = response
        .get("ts")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let output = json!({
        "ok": true,
        "channel_id": channel_id,
        "ts": ts,
    });

    println!("{}", to_json_output(&output));
    Ok(())
}
