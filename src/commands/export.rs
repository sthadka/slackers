use crate::auth::resolve_auth;
use crate::error::Result;
use crate::render::export::render_html_export;
use crate::slack::{resolve_channel_id, to_compact_message, CompactMessageOptions, SlackClient};
use serde_json::Value;
use std::time::Duration;

/// Fetch the full message history for `channel` and write it in the chosen `format`.
///
/// # Parameters
/// - `client`        – authenticated Slack API client
/// - `channel`       – channel name or ID (e.g. `#general` or `C0123ABCD`)
/// - `format`        – `"json"` | `"csv"` | `"html"`
/// - `output`        – write to this file path; if `None`, write to stdout
/// - `workspace_url` – workspace base URL used only for resolving auth (may be empty)
pub async fn handle_export(
    client: &SlackClient,
    channel: &str,
    format: &str,
    output: Option<&str>,
    workspace_url: &str,
) -> Result<()> {
    let _ = workspace_url; // consumed by the caller; kept in signature for future use

    let channel_id = resolve_channel_id(client, channel).await?;

    // ── Fetch all history pages ───────────────────────────────────────────
    let compact_options = CompactMessageOptions {
        max_content_chars: None, // export everything
        include_thread_ts: true,
    };

    let mut messages: Vec<Value> = Vec::new();
    let mut page_cursor: Option<String> = None;

    loop {
        let mut params = vec![
            ("channel".to_string(), channel_id.clone()),
            ("limit".to_string(), "200".to_string()),
        ];
        if let Some(ref c) = page_cursor {
            params.push(("cursor".to_string(), c.clone()));
        }

        let response = client.api_call("conversations.history", params).await?;

        let page: Vec<Value> = response
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().cloned().collect())
            .unwrap_or_default();

        for raw in page {
            let compact = to_compact_message(&raw, &compact_options);
            messages.push(serde_json::to_value(compact)?);
        }

        page_cursor = response
            .get("response_metadata")
            .and_then(|m| m.get("next_cursor"))
            .and_then(|c| c.as_str())
            .filter(|c| !c.is_empty())
            .map(|c| c.to_string());

        if page_cursor.is_none() {
            break;
        }

        // Stay under Slack Tier-3 limit (50 req/min).
        tokio::time::sleep(Duration::from_millis(1200)).await;
    }

    // Slack returns history newest-first; reverse so oldest is first.
    messages.reverse();

    let count = messages.len();

    // ── Render ────────────────────────────────────────────────────────────
    let content = match format {
        "json" => serde_json::to_string_pretty(&messages)?,
        "csv" => render_csv(&messages),
        "html" => render_html_export(&messages),
        other => {
            return Err(crate::error::SlackersError::Other(format!(
                "Unknown export format '{}'; supported: json, csv, html",
                other
            ))
            .into())
        }
    };

    // ── Write output ──────────────────────────────────────────────────────
    match output {
        Some(path) => {
            std::fs::write(path, &content)?;
        }
        None => {
            print!("{}", content);
        }
    }

    eprintln!("Exported {} messages", count);
    Ok(())
}

/// Build a CSV string from compact message values.
///
/// Columns: timestamp, user, text, reactions
fn render_csv(messages: &[Value]) -> String {
    let mut out = String::from("timestamp,user,text,reactions\n");
    for msg in messages {
        let ts = csv_field(msg.get("ts").and_then(|v| v.as_str()).unwrap_or(""));
        let user = csv_field(msg.get("user").and_then(|v| v.as_str()).unwrap_or(""));
        let text = csv_field(msg.get("text").and_then(|v| v.as_str()).unwrap_or(""));
        let reactions = {
            let r = msg
                .get("reactions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|r| {
                            let name = r.get("name").and_then(|v| v.as_str())?;
                            let count = r.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
                            Some(format!(":{}:{}", name, count))
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            csv_field(&r)
        };
        out.push_str(&format!("{},{},{},{}\n", ts, user, text, reactions));
    }
    out
}

/// Wrap a field in double-quotes, escaping any embedded double-quotes.
fn csv_field(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Convenience entry point that resolves auth internally, intended for use
/// from the CLI dispatch layer.
///
/// `workspace` is the optional `--workspace` flag value.
pub async fn run_export(
    channel: &str,
    format: &str,
    output: Option<&str>,
    workspace: Option<&str>,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let workspace_url = auth_result.workspace_url.clone().unwrap_or_default();
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);
    handle_export(&client, channel, format, output, &workspace_url).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_csv_field_plain() {
        assert_eq!(csv_field("hello"), "\"hello\"");
    }

    #[test]
    fn test_csv_field_with_quotes() {
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_render_csv_headers() {
        let csv = render_csv(&[]);
        assert_eq!(csv, "timestamp,user,text,reactions\n");
    }

    #[test]
    fn test_render_csv_single_message() {
        let msgs = vec![json!({
            "ts": "1609459200.000000",
            "user": "U123",
            "text": "Hello world",
            "reactions": [{"name": "thumbsup", "count": 3}]
        })];
        let csv = render_csv(&msgs);
        assert!(csv.contains("1609459200.000000"));
        assert!(csv.contains("U123"));
        assert!(csv.contains("Hello world"));
        assert!(csv.contains(":thumbsup:3"));
    }

    #[test]
    fn test_render_csv_no_reactions() {
        let msgs = vec![json!({
            "ts": "1609459200.000000",
            "user": "U123",
            "text": "No reactions here"
        })];
        let csv = render_csv(&msgs);
        // reactions column should be empty quoted field
        assert!(csv.ends_with(",\"\"\n"));
    }

    #[test]
    fn test_render_csv_text_with_comma() {
        let msgs = vec![json!({
            "ts": "1.0",
            "user": "U1",
            "text": "hello, world"
        })];
        let csv = render_csv(&msgs);
        assert!(csv.contains("\"hello, world\""));
    }
}
