use crate::auth::resolve_auth;
use crate::cli::{MessageScheduleOptions, MessageScheduledDeleteOptions, MessageScheduledListOptions, ScheduledCommand};
use crate::error::{Result, SlackersError};
use crate::output::to_json_output;
use crate::slack::SlackClient;
use serde_json::json;

pub async fn handle_scheduled(subcommand: ScheduledCommand) -> Result<()> {
    match subcommand {
        ScheduledCommand::Send(opts) => handle_scheduled_send(opts).await,
        ScheduledCommand::List(opts) => handle_scheduled_list(opts).await,
        ScheduledCommand::Delete(opts) => handle_scheduled_delete(opts).await,
    }
}

async fn handle_scheduled_send(opts: MessageScheduleOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Parse --at as unix timestamp (integer) or RFC3339 datetime
    let post_at = parse_post_at(&opts.at)?;

    let body = client.schedule_message(&opts.channel, &opts.message, post_at).await?;

    let output = if crate::output::is_quiet() {
        json!({ "ok": true })
    } else {
        let scheduled_message_id = body
            .get("scheduled_message_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        json!({
            "ok": true,
            "scheduled_message_id": scheduled_message_id,
            "post_at": post_at,
        })
    };
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_scheduled_list(opts: MessageScheduledListOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Optionally resolve channel name → ID
    let channel_id = match &opts.channel {
        Some(ch) => {
            let id = crate::slack::channels::resolve_channel_id(&client, ch).await?;
            Some(id)
        }
        None => None,
    };

    let messages = client.list_scheduled_messages(channel_id.as_deref()).await?;

    let output = json!(messages);
    println!("{}", to_json_output(&output));

    Ok(())
}

async fn handle_scheduled_delete(opts: MessageScheduledDeleteOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    // Resolve channel name → ID
    let channel_id = crate::slack::channels::resolve_channel_id(&client, &opts.channel).await?;

    client.delete_scheduled_message(&channel_id, &opts.id).await?;

    let output = json!({ "ok": true });
    println!("{}", to_json_output(&output));

    Ok(())
}

/// Parse `--at` value as either a unix timestamp integer or an RFC3339 string.
///
/// Supported formats:
/// - Plain integer: `1700000000`
/// - RFC3339: `2024-11-14T22:13:20+00:00` or `2024-11-14T22:13:20Z`
fn parse_post_at(at: &str) -> Result<i64> {
    // Try integer first
    if let Ok(ts) = at.parse::<i64>() {
        return Ok(ts);
    }

    // Try RFC3339 — minimal parser for Z and ±HH:MM offsets
    parse_rfc3339(at).ok_or_else(|| {
        SlackersError::Other(format!(
            "Invalid --at value '{}': expected a unix timestamp (integer) or RFC3339 datetime (e.g. 2024-11-14T22:13:20Z)",
            at
        ))
    })
}

/// Minimal RFC3339 / ISO 8601 parser — handles YYYY-MM-DDTHH:MM:SS[Z|±HH:MM].
fn parse_rfc3339(s: &str) -> Option<i64> {
    // Expect at least: 2024-11-14T22:13:20
    if s.len() < 19 {
        return None;
    }

    let (date_time_str, offset_str) = if s.ends_with('Z') {
        (&s[..s.len() - 1], "+00:00")
    } else if s.len() >= 25 {
        let (dt, off) = s.split_at(19);
        (dt, off)
    } else {
        return None;
    };

    let parts: Vec<&str> = date_time_str.splitn(2, 'T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<i64> = parts[0].split('-')
        .filter_map(|p| p.parse().ok())
        .collect();
    let time_parts: Vec<i64> = parts[1].split(':')
        .filter_map(|p| p.parse().ok())
        .collect();

    if date_parts.len() < 3 || time_parts.len() < 3 {
        return None;
    }

    let year = date_parts[0];
    let month = date_parts[1];
    let day = date_parts[2];
    let hour = time_parts[0];
    let minute = time_parts[1];
    let second = time_parts[2];

    // Compute days since Unix epoch (1970-01-01)
    let days = days_from_epoch(year, month, day)?;
    let secs = days * 86400 + hour * 3600 + minute * 60 + second;

    // Parse offset ±HH:MM and subtract to get UTC
    let offset_secs = parse_offset(offset_str)?;

    Some(secs - offset_secs)
}

fn parse_offset(s: &str) -> Option<i64> {
    if s == "+00:00" || s == "-00:00" {
        return Some(0);
    }
    let sign: i64 = if s.starts_with('+') { 1 } else if s.starts_with('-') { -1 } else { return None; };
    let rest = &s[1..];
    let p: Vec<i64> = rest.split(':').filter_map(|x| x.parse().ok()).collect();
    if p.len() < 2 {
        return None;
    }
    Some(sign * (p[0] * 3600 + p[1] * 60))
}

fn days_from_epoch(year: i64, month: i64, day: i64) -> Option<i64> {
    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days in each month (non-leap); index 0 unused
    let days_per_month: [i64; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);

    let mut days: i64 = 0;
    // Add full years since epoch
    for y in 1970..year {
        days += if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) { 366 } else { 365 };
    }
    // Add full months in the current year
    for m in 1..month {
        days += days_per_month[m as usize];
        if m == 2 && is_leap {
            days += 1;
        }
    }
    days += day - 1;
    Some(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_post_at_integer() {
        assert_eq!(parse_post_at("1700000000").unwrap(), 1700000000);
    }

    #[test]
    fn test_parse_post_at_rfc3339_utc() {
        // 2023-11-14T22:13:20Z  = 1700000000
        assert_eq!(parse_post_at("2023-11-14T22:13:20Z").unwrap(), 1700000000);
    }

    #[test]
    fn test_parse_post_at_rfc3339_offset() {
        // 2023-11-14T23:13:20+01:00 = same UTC as above
        assert_eq!(parse_post_at("2023-11-14T23:13:20+01:00").unwrap(), 1700000000);
    }

    #[test]
    fn test_parse_post_at_invalid() {
        assert!(parse_post_at("not-a-date").is_err());
    }
}
