use crate::auth::resolve_auth;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::SlackClient;
use serde_json::json;

/// Handle `slackers workspace info --workspace <url>`
///
/// Calls `team.info` and emits a JSON object with workspace id, name, domain,
/// and the largest available icon URL.
pub async fn handle_workspace_info(workspace: Option<&str>) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let info = client.get_workspace_info().await?;

    // Pick the largest available icon URL
    let icon_url = info.icon.as_ref().and_then(|ic| {
        ic.image_230
            .as_deref()
            .or(ic.image_132.as_deref())
            .or(ic.image_102.as_deref())
            .or(ic.image_88.as_deref())
            .or(ic.image_68.as_deref())
            .or(ic.image_44.as_deref())
            .or(ic.image_34.as_deref())
            .map(|s| s.to_string())
    });

    let output = json!({
        "id": info.id,
        "name": info.name,
        "domain": info.domain,
        "icon_url": icon_url,
    });

    println!("{}", to_json_output(&output));
    Ok(())
}

/// Handle `slackers emoji list --workspace <url>`
///
/// Calls `emoji.list` and emits a JSON object mapping emoji names to URLs
/// (or alias strings like `"alias:other_emoji"`).
pub async fn handle_emoji_list(workspace: Option<&str>) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let emoji_map = client.list_emojis().await?;

    let output = json!(emoji_map);
    println!("{}", to_json_output(&output));
    Ok(())
}
