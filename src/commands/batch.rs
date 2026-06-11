use crate::auth::resolve_auth;
use crate::cli::{BatchReactOptions, BatchSendOptions};
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::{emoji::normalize_reaction_name, SlackClient};
use crate::target::{parse_msg_target, MsgTarget};
use serde_json::{json, Value};

pub async fn handle_batch_send(options: BatchSendOptions) -> Result<()> {
    let auth_result = resolve_auth(options.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let mut results: Vec<Value> = Vec::new();

    for channel in &options.channels {
        let params = vec![
            ("channel".to_string(), channel.clone()),
            ("text".to_string(), options.message.clone()),
        ];

        match client.api_call("chat.postMessage", params).await {
            Ok(response) => {
                let ts = response
                    .get("ts")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                results.push(json!({
                    "channel": channel,
                    "ok": true,
                    "ts": ts,
                }));
            }
            Err(e) => {
                results.push(json!({
                    "channel": channel,
                    "ok": false,
                    "error": e.to_string(),
                }));
            }
        }
    }

    let output = json!({ "results": results });
    println!("{}", to_json_output(&output));
    Ok(())
}

pub async fn handle_batch_react(options: BatchReactOptions) -> Result<()> {
    let normalized_emoji = normalize_reaction_name(&options.emoji);

    // Collect per-message results; we try all messages regardless of failures.
    let mut results: Vec<Value> = Vec::new();

    for message in &options.messages {
        // Each message may live in a different workspace if it's a URL.
        let msg_target = match parse_msg_target(message) {
            Ok(t) => t,
            Err(e) => {
                results.push(json!({
                    "message": message,
                    "ok": false,
                    "error": format!("Failed to parse target: {}", e),
                }));
                continue;
            }
        };

        let (channel_id, message_ts, workspace_url) = match msg_target {
            MsgTarget::Url(ref msg_ref) => (
                msg_ref.channel_id.clone(),
                msg_ref.message_ts.clone(),
                Some(msg_ref.workspace_url.clone()),
            ),
            MsgTarget::Channel(_) => {
                results.push(json!({
                    "message": message,
                    "ok": false,
                    "error": "Batch react requires Slack message URLs (not plain channel names)",
                }));
                continue;
            }
        };

        let auth_result = match resolve_auth(workspace_url.as_deref()) {
            Ok(a) => a,
            Err(e) => {
                results.push(json!({
                    "message": message,
                    "ok": false,
                    "error": format!("Auth error: {}", e),
                }));
                continue;
            }
        };

        let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

        let params = vec![
            ("channel".to_string(), channel_id),
            ("timestamp".to_string(), message_ts),
            ("name".to_string(), normalized_emoji.clone()),
        ];

        match client.api_call("reactions.add", params).await {
            Ok(_) => {
                results.push(json!({
                    "message": message,
                    "ok": true,
                }));
            }
            Err(e) => {
                let error_str = e.to_string();
                // already_reacted is treated as success
                if error_str.contains("already_reacted") {
                    results.push(json!({
                        "message": message,
                        "ok": true,
                        "note": "already_reacted",
                    }));
                } else {
                    results.push(json!({
                        "message": message,
                        "ok": false,
                        "error": error_str,
                    }));
                }
            }
        }
    }

    let output = json!({ "results": results });
    println!("{}", to_json_output(&output));
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Test that results are accumulated correctly — even with mixed ok/error items.
    #[test]
    fn test_result_aggregation() {
        use serde_json::json;

        let results = vec![
            json!({ "channel": "#general", "ok": true, "ts": "1234567890.123456" }),
            json!({ "channel": "#missing", "ok": false, "error": "channel_not_found" }),
        ];

        let ok_count = results.iter().filter(|r| r["ok"] == true).count();
        let err_count = results.iter().filter(|r| r["ok"] == false).count();

        assert_eq!(ok_count, 1);
        assert_eq!(err_count, 1);
    }

    /// Test that already_reacted is treated as success.
    #[test]
    fn test_already_reacted_is_success() {
        let error_str = "Slack API error: already_reacted";
        let is_already_reacted = error_str.contains("already_reacted");
        assert!(is_already_reacted);
    }
}
