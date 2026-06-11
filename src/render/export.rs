use crate::slack::CompactSlackMessage;
use serde_json::Value;

/// Render a list of messages (and their optional thread replies) as an HTML page.
///
/// Each element of `messages` is expected to be a `serde_json::Value` produced by
/// `to_compact_message` (i.e. fields: `ts`, `user`, `text`, optional `thread`
/// array of reply values with the same shape).
///
/// No external crates are used — HTML is built with `format!()` calls.
pub fn render_html_export(messages: &[Value]) -> String {
    let mut body = String::new();

    for msg in messages {
        body.push_str(&render_message(msg, false));

        // Render thread replies indented
        if let Some(replies) = msg.get("thread").and_then(|v| v.as_array()) {
            for reply in replies {
                body.push_str(&render_message(reply, true));
            }
        }
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Slack Export</title>
  <style>
    /* ── Reset / base ───────────────────────────────────── */
    *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
                   Oxygen, Ubuntu, sans-serif;
      background: #1a1d21;
      color: #d1d2d3;
      line-height: 1.5;
      padding: 0;
    }}

    /* ── Layout ─────────────────────────────────────────── */
    .channel-view {{
      max-width: 860px;
      margin: 0 auto;
      padding: 24px 16px 80px;
    }}

    /* ── Message ─────────────────────────────────────────── */
    .message {{
      display: flex;
      gap: 12px;
      padding: 6px 8px;
      border-radius: 4px;
      margin-bottom: 2px;
      transition: background 0.1s;
    }}
    .message:hover {{ background: #222529; }}

    .avatar {{
      flex-shrink: 0;
      width: 36px;
      height: 36px;
      border-radius: 4px;
      background: #4a154b;
      color: #fff;
      font-weight: 700;
      font-size: 14px;
      display: flex;
      align-items: center;
      justify-content: center;
      text-transform: uppercase;
      user-select: none;
    }}

    .message-body {{ flex: 1; min-width: 0; }}

    .message-header {{
      display: flex;
      align-items: baseline;
      gap: 8px;
      flex-wrap: wrap;
    }}

    .username {{
      font-weight: 700;
      font-size: 0.93rem;
      color: #e8e8e8;
    }}

    .timestamp {{
      font-size: 0.75rem;
      color: #868686;
    }}

    .text {{
      font-size: 0.9rem;
      color: #c9c9c9;
      margin-top: 2px;
      white-space: pre-wrap;
      word-break: break-word;
    }}

    /* ── Thread replies ──────────────────────────────────── */
    .thread-reply {{
      margin-left: 48px;
      border-left: 2px solid #333;
      padding-left: 12px;
    }}
    .thread-reply .avatar {{
      width: 28px;
      height: 28px;
      font-size: 11px;
    }}
    .thread-reply .message {{ padding: 4px 8px; }}

    /* ── Reactions ───────────────────────────────────────── */
    .reactions {{
      display: flex;
      flex-wrap: wrap;
      gap: 4px;
      margin-top: 4px;
    }}
    .reaction {{
      background: #2c2d30;
      border: 1px solid #3a3b3e;
      border-radius: 12px;
      padding: 1px 7px;
      font-size: 0.8rem;
      color: #a8a8a8;
    }}
  </style>
</head>
<body>
  <div class="channel-view">
{}  </div>
</body>
</html>"#,
        body
    )
}

fn render_message(msg: &Value, is_reply: bool) -> String {
    let ts = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");
    let user = msg
        .get("user")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let text = msg.get("text").and_then(|v| v.as_str()).unwrap_or("");

    // Avatar: first letter of user id or display name
    let avatar_char = user.chars().next().unwrap_or('?');

    // Human-readable timestamp (best-effort parse of Slack's Unix ts)
    let readable_ts = format_ts(ts);

    // Reactions
    let reactions_html = render_reactions(msg);

    let wrapper_class = if is_reply {
        "thread-reply"
    } else {
        ""
    };

    let avatar_color = avatar_color_for(user);

    format!(
        r#"    <div class="{wrapper}">
      <div class="message">
        <div class="avatar" style="background:{color};">{avatar}</div>
        <div class="message-body">
          <div class="message-header">
            <span class="username">{user}</span>
            <span class="timestamp">{ts}</span>
          </div>
          <div class="text">{text}</div>
          {reactions}
        </div>
      </div>
    </div>
"#,
        wrapper = wrapper_class,
        color = avatar_color,
        avatar = escape_html(&avatar_char.to_string()),
        user = escape_html(user),
        ts = escape_html(&readable_ts),
        text = escape_html(text),
        reactions = reactions_html,
    )
}

fn render_reactions(msg: &Value) -> String {
    let reactions = match msg.get("reactions").and_then(|v| v.as_array()) {
        Some(r) if !r.is_empty() => r,
        _ => return String::new(),
    };

    let mut pills = String::new();
    for r in reactions {
        let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let count = r.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
        pills.push_str(&format!(
            r#"<span class="reaction">:{}: {}</span>"#,
            escape_html(name),
            count
        ));
    }

    format!(r#"<div class="reactions">{}</div>"#, pills)
}

/// Convert a Slack timestamp ("1234567890.123456") to a readable string.
/// Falls back to the raw ts if parsing fails.
fn format_ts(ts: &str) -> String {
    let secs: u64 = ts
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if secs == 0 {
        return ts.to_string();
    }

    // Manual UTC formatting without external crates
    // Days since Unix epoch → year/month/day
    let (y, mo, d, h, mi, s) = epoch_to_ymd_hms(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, mo, d, h, mi, s)
}

/// Convert Unix seconds to (year, month, day, hour, min, sec) in UTC.
fn epoch_to_ymd_hms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = secs % 60;
    let min = (secs / 60) % 60;
    let hour = (secs / 3600) % 24;
    let days = secs / 86400;

    // Days since 1970-01-01 → calendar date (Gregorian)
    let (y, mo, d) = days_to_ymd(days);
    (y, mo, d, hour, min, sec)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Reference: 1970-01-01 = day 0
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month = 1u64;
    for md in &month_days {
        if days < *md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Escape characters that have special meaning in HTML.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Deterministic background colour for a user avatar based on user id.
fn avatar_color_for(user: &str) -> &'static str {
    const COLORS: &[&str] = &[
        "#4a154b", // Slack purple
        "#1264a3", // Slack blue
        "#007a5a", // Slack green
        "#e01e5a", // Slack red
        "#ecb22e", // Slack yellow (darker bg)
        "#36c5f0", // Slack sky blue
        "#2eb67d", // Slack teal
        "#e8912d", // Slack orange
    ];
    let hash: usize = user.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize));
    COLORS[hash % COLORS.len()]
}

/// Build an HTML export from `CompactSlackMessage` structs directly.
///
/// This is a convenience wrapper over `render_html_export` for callers that
/// already have typed `CompactSlackMessage` values.
pub fn render_html_from_compact(messages: &[CompactSlackMessage]) -> String {
    let values: Vec<Value> = messages
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();
    render_html_export(&values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<b>Test & \"foo\"</b>"), "&lt;b&gt;Test &amp; &quot;foo&quot;&lt;/b&gt;");
    }

    #[test]
    fn test_render_html_export_empty() {
        let html = render_html_export(&[]);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Slack Export"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_render_html_export_single_message() {
        let msgs = vec![json!({
            "ts": "1609459200.000000",
            "user": "U123",
            "text": "Hello world"
        })];
        let html = render_html_export(&msgs);
        assert!(html.contains("Hello world"));
        assert!(html.contains("U123"));
        assert!(html.contains("2021-01-01"));
    }

    #[test]
    fn test_render_html_export_with_thread() {
        let msgs = vec![json!({
            "ts": "1609459200.000000",
            "user": "U123",
            "text": "Root message",
            "thread": [
                {"ts": "1609459300.000000", "user": "U456", "text": "Reply 1"}
            ]
        })];
        let html = render_html_export(&msgs);
        assert!(html.contains("Root message"));
        assert!(html.contains("Reply 1"));
        assert!(html.contains("thread-reply"));
    }

    #[test]
    fn test_render_html_export_with_reactions() {
        let msgs = vec![json!({
            "ts": "1609459200.000000",
            "user": "U123",
            "text": "Great!",
            "reactions": [{"name": "thumbsup", "count": 3}]
        })];
        let html = render_html_export(&msgs);
        assert!(html.contains(":thumbsup:"));
        assert!(html.contains("3"));
    }

    #[test]
    fn test_format_ts_known_epoch() {
        // 2021-01-01 00:00:00 UTC = 1609459200
        assert_eq!(format_ts("1609459200.000000"), "2021-01-01 00:00:00 UTC");
    }

    #[test]
    fn test_avatar_color_deterministic() {
        let c1 = avatar_color_for("U123");
        let c2 = avatar_color_for("U123");
        assert_eq!(c1, c2);
    }
}
