use crate::cli::ReportCommand;
use crate::error::Result;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Parse a period string like "30d" into epoch seconds for the cutoff.
fn period_to_cutoff(period: &str) -> Result<String> {
    let s = period.trim();
    let num_str = if s.ends_with('d') || s.ends_with('D') {
        &s[..s.len() - 1]
    } else {
        s
    };
    let days: u64 = num_str.parse().map_err(|_| {
        crate::error::SlackersError::Store(format!(
            "Invalid period '{}'. Expected format like '30d'.",
            period
        ))
    })?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    Ok(format!("{:.6}", now - (days as f64 * 86400.0)))
}

pub async fn handle_report(cmd: ReportCommand) -> Result<()> {
    let resolved = crate::auth::resolve_auth(None)?;
    let workspace_url = resolved.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;

    match cmd {
        ReportCommand::Activity(opts) => report_activity(&db_path, opts).await,
        ReportCommand::User(opts) => report_user(&db_path, opts).await,
        ReportCommand::Threads(opts) => report_threads(&db_path, opts).await,
        ReportCommand::Reactions(opts) => report_reactions(&db_path, opts).await,
        ReportCommand::Mentions(opts) => report_mentions(&db_path, opts).await,
    }
}

#[derive(Serialize)]
struct ActivityReport {
    channel: Option<String>,
    period: Option<String>,
    total_messages: i64,
    unique_posters: i64,
    messages_per_day: f64,
    thread_ratio: f64,
    daily_breakdown: Vec<DailyActivity>,
    peak_hours: Vec<HourActivity>,
}

#[derive(Serialize)]
struct DailyActivity {
    date: String,
    message_count: i64,
}

#[derive(Serialize)]
struct HourActivity {
    hour: i64,
    message_count: i64,
}

async fn report_activity(
    db_path: &std::path::Path,
    opts: crate::cli::ReportActivityOpts,
) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;

    let mut where_clauses = vec!["is_deleted = 0".to_string()];
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref channel) = opts.channel {
        where_clauses.push(format!("channel_id = ?{}", idx));
        param_values.push(Box::new(channel.clone()));
        idx += 1;
    }
    if let Some(ref period) = opts.period {
        let cutoff = period_to_cutoff(period)?;
        where_clauses.push(format!("ts >= ?{}", idx));
        param_values.push(Box::new(cutoff));
        idx += 1;
    }

    let where_sql = where_clauses.join(" AND ");

    // Total messages and unique posters
    let summary_sql = format!(
        "SELECT COUNT(*), COUNT(DISTINCT user_id) FROM messages WHERE {}",
        where_sql
    );
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let (total_messages, unique_posters): (i64, i64) =
        conn.query_row(&summary_sql, params_refs.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

    // Thread ratio
    let thread_sql = format!(
        "SELECT COUNT(*) FROM messages WHERE {} AND thread_ts IS NOT NULL",
        where_sql
    );
    let threaded: i64 = conn.query_row(&thread_sql, params_refs.as_slice(), |row| row.get(0))?;
    let thread_ratio = if total_messages > 0 {
        threaded as f64 / total_messages as f64
    } else {
        0.0
    };

    // Daily breakdown
    let daily_sql = format!(
        "SELECT DATE(CAST(ts AS REAL), 'unixepoch') AS day, COUNT(*) AS cnt
         FROM messages WHERE {}
         GROUP BY day ORDER BY day DESC LIMIT ?{}",
        where_sql, idx
    );
    let mut daily_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let daily_limit: u32 = 90;
    daily_params.push(&daily_limit);

    let mut stmt = conn.prepare(&daily_sql)?;
    let daily_rows = stmt.query_map(daily_params.as_slice(), |row| {
        Ok(DailyActivity {
            date: row.get(0)?,
            message_count: row.get(1)?,
        })
    })?;
    let mut daily_breakdown = Vec::new();
    for row in daily_rows {
        daily_breakdown.push(row?);
    }

    let num_days = daily_breakdown.len().max(1) as f64;
    let messages_per_day = total_messages as f64 / num_days;

    // Peak hours
    let hour_sql = format!(
        "SELECT CAST(strftime('%H', CAST(ts AS REAL), 'unixepoch') AS INTEGER) AS hour, COUNT(*) AS cnt
         FROM messages WHERE {}
         GROUP BY hour ORDER BY cnt DESC LIMIT ?{}",
        where_sql, idx
    );
    let mut hour_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let hour_limit: u32 = 24;
    hour_params.push(&hour_limit);

    let mut stmt = conn.prepare(&hour_sql)?;
    let hour_rows = stmt.query_map(hour_params.as_slice(), |row| {
        Ok(HourActivity {
            hour: row.get(0)?,
            message_count: row.get(1)?,
        })
    })?;
    let mut peak_hours = Vec::new();
    for row in hour_rows {
        peak_hours.push(row?);
    }

    let report = ActivityReport {
        channel: opts.channel,
        period: opts.period,
        total_messages,
        unique_posters,
        messages_per_day,
        thread_ratio,
        daily_breakdown,
        peak_hours,
    };

    println!("{}", crate::output::to_json_output(&report));
    Ok(())
}

#[derive(Serialize)]
struct UserReport {
    user: Option<String>,
    period: Option<String>,
    total_messages: i64,
    channels_active: i64,
    threads_participated: i64,
    avg_thread_response_time_secs: Option<f64>,
    channel_breakdown: Vec<ChannelMessageCount>,
}

#[derive(Serialize)]
struct ChannelMessageCount {
    channel_id: String,
    message_count: i64,
}

async fn report_user(
    db_path: &std::path::Path,
    opts: crate::cli::ReportUserOpts,
) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;

    let mut where_clauses = vec!["is_deleted = 0".to_string()];
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref user) = opts.user {
        where_clauses.push(format!("user_id = ?{}", idx));
        param_values.push(Box::new(user.clone()));
        idx += 1;
    }
    if let Some(ref period) = opts.period {
        let cutoff = period_to_cutoff(period)?;
        where_clauses.push(format!("ts >= ?{}", idx));
        param_values.push(Box::new(cutoff));
        idx += 1;
    }

    let where_sql = where_clauses.join(" AND ");
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    // Total messages and channels active
    let summary_sql = format!(
        "SELECT COUNT(*), COUNT(DISTINCT channel_id) FROM messages WHERE {}",
        where_sql
    );
    let (total_messages, channels_active): (i64, i64) =
        conn.query_row(&summary_sql, params_refs.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

    // Threads participated
    let thread_sql = format!(
        "SELECT COUNT(DISTINCT thread_ts) FROM messages WHERE {} AND thread_ts IS NOT NULL",
        where_sql
    );
    let threads_participated: i64 =
        conn.query_row(&thread_sql, params_refs.as_slice(), |row| row.get(0))?;

    // Average thread response time: for threads started by someone else where this user replied,
    // compute the average time between thread_ts and the user's first reply.
    let avg_response_sql = format!(
        "SELECT AVG(CAST(m.ts AS REAL) - CAST(m.thread_ts AS REAL))
         FROM messages m
         WHERE {} AND m.thread_ts IS NOT NULL AND m.ts != m.thread_ts",
        where_sql
    );
    let avg_thread_response_time_secs: Option<f64> = conn
        .query_row(&avg_response_sql, params_refs.as_slice(), |row| row.get(0))
        .unwrap_or(None);

    // Channel breakdown
    let channel_sql = format!(
        "SELECT channel_id, COUNT(*) AS cnt FROM messages WHERE {} GROUP BY channel_id ORDER BY cnt DESC LIMIT ?{}",
        where_sql, idx
    );
    let mut channel_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let chan_limit: u32 = 50;
    channel_params.push(&chan_limit);

    let mut stmt = conn.prepare(&channel_sql)?;
    let rows = stmt.query_map(channel_params.as_slice(), |row| {
        Ok(ChannelMessageCount {
            channel_id: row.get(0)?,
            message_count: row.get(1)?,
        })
    })?;
    let mut channel_breakdown = Vec::new();
    for row in rows {
        channel_breakdown.push(row?);
    }

    let report = UserReport {
        user: opts.user,
        period: opts.period,
        total_messages,
        channels_active,
        threads_participated,
        avg_thread_response_time_secs,
        channel_breakdown,
    };

    println!("{}", crate::output::to_json_output(&report));
    Ok(())
}

#[derive(Serialize)]
struct ThreadsReport {
    channel: Option<String>,
    period: Option<String>,
    total_threads: i64,
    longest_threads: Vec<ThreadSummary>,
    unanswered_threads: Vec<ThreadSummary>,
}

#[derive(Serialize)]
struct ThreadSummary {
    channel_id: String,
    thread_ts: String,
    reply_count: i64,
    participant_count: i64,
    first_reply: Option<String>,
    last_reply: Option<String>,
}

async fn report_threads(
    db_path: &std::path::Path,
    opts: crate::cli::ReportThreadsOpts,
) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;

    let mut where_clauses = vec!["is_deleted = 0".to_string(), "thread_ts IS NOT NULL".to_string()];
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref channel) = opts.channel {
        where_clauses.push(format!("channel_id = ?{}", idx));
        param_values.push(Box::new(channel.clone()));
        idx += 1;
    }
    if let Some(ref period) = opts.period {
        let cutoff = period_to_cutoff(period)?;
        where_clauses.push(format!("ts >= ?{}", idx));
        param_values.push(Box::new(cutoff));
        idx += 1;
    }

    let where_sql = where_clauses.join(" AND ");
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    // Total unique threads
    let total_sql = format!(
        "SELECT COUNT(DISTINCT thread_ts) FROM messages WHERE {}",
        where_sql
    );
    let total_threads: i64 =
        conn.query_row(&total_sql, params_refs.as_slice(), |row| row.get(0))?;

    // Longest threads (most replies)
    let longest_sql = format!(
        "SELECT channel_id, thread_ts,
                COUNT(*) AS reply_count,
                COUNT(DISTINCT user_id) AS participant_count,
                MIN(ts) AS first_reply,
                MAX(ts) AS last_reply
         FROM messages WHERE {}
         GROUP BY channel_id, thread_ts
         ORDER BY reply_count DESC
         LIMIT ?{}",
        where_sql, idx
    );
    let mut longest_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let top_limit: u32 = 10;
    longest_params.push(&top_limit);

    let mut stmt = conn.prepare(&longest_sql)?;
    let rows = stmt.query_map(longest_params.as_slice(), |row| {
        Ok(ThreadSummary {
            channel_id: row.get(0)?,
            thread_ts: row.get(1)?,
            reply_count: row.get(2)?,
            participant_count: row.get(3)?,
            first_reply: row.get(4)?,
            last_reply: row.get(5)?,
        })
    })?;
    let mut longest_threads = Vec::new();
    for row in rows {
        longest_threads.push(row?);
    }

    // Unanswered threads: threads where reply_count = 1 (just the parent, no replies)
    // A thread is unanswered if only 1 message has that thread_ts
    let unanswered_sql = format!(
        "SELECT channel_id, thread_ts,
                COUNT(*) AS reply_count,
                COUNT(DISTINCT user_id) AS participant_count,
                MIN(ts) AS first_reply,
                MAX(ts) AS last_reply
         FROM messages WHERE {}
         GROUP BY channel_id, thread_ts
         HAVING COUNT(*) = 1
         ORDER BY thread_ts DESC
         LIMIT ?{}",
        where_sql, idx
    );
    let mut unanswered_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let unanswered_limit: u32 = 10;
    unanswered_params.push(&unanswered_limit);

    let mut stmt = conn.prepare(&unanswered_sql)?;
    let rows = stmt.query_map(unanswered_params.as_slice(), |row| {
        Ok(ThreadSummary {
            channel_id: row.get(0)?,
            thread_ts: row.get(1)?,
            reply_count: row.get(2)?,
            participant_count: row.get(3)?,
            first_reply: row.get(4)?,
            last_reply: row.get(5)?,
        })
    })?;
    let mut unanswered_threads = Vec::new();
    for row in rows {
        unanswered_threads.push(row?);
    }

    let report = ThreadsReport {
        channel: opts.channel,
        period: opts.period,
        total_threads,
        longest_threads,
        unanswered_threads,
    };

    println!("{}", crate::output::to_json_output(&report));
    Ok(())
}

#[derive(Serialize)]
struct ReactionsReport {
    channel: Option<String>,
    period: Option<String>,
    total_reactions: i64,
    top_emoji: Vec<EmojiCount>,
    most_reacted_messages: Vec<ReactedMessage>,
}

#[derive(Serialize)]
struct EmojiCount {
    emoji: String,
    count: i64,
}

#[derive(Serialize)]
struct ReactedMessage {
    channel_id: String,
    message_ts: String,
    reaction_count: i64,
}

async fn report_reactions(
    db_path: &std::path::Path,
    opts: crate::cli::ReportReactionsOpts,
) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;

    let mut where_clauses: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref channel) = opts.channel {
        where_clauses.push(format!("r.channel_id = ?{}", idx));
        param_values.push(Box::new(channel.clone()));
        idx += 1;
    }
    if let Some(ref period) = opts.period {
        let cutoff = period_to_cutoff(period)?;
        where_clauses.push(format!("r.message_ts >= ?{}", idx));
        param_values.push(Box::new(cutoff));
        idx += 1;
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_clauses.join(" AND "))
    };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    // Total reactions
    let total_sql = format!("SELECT COUNT(*) FROM reactions r{}", where_sql);
    let total_reactions: i64 =
        conn.query_row(&total_sql, params_refs.as_slice(), |row| row.get(0))?;

    // Top emoji
    let emoji_sql = format!(
        "SELECT r.emoji, COUNT(*) AS cnt FROM reactions r{} GROUP BY r.emoji ORDER BY cnt DESC LIMIT ?{}",
        where_sql, idx
    );
    let mut emoji_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let emoji_limit: u32 = 20;
    emoji_params.push(&emoji_limit);

    let mut stmt = conn.prepare(&emoji_sql)?;
    let rows = stmt.query_map(emoji_params.as_slice(), |row| {
        Ok(EmojiCount {
            emoji: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    let mut top_emoji = Vec::new();
    for row in rows {
        top_emoji.push(row?);
    }

    // Most reacted messages
    let msg_sql = format!(
        "SELECT r.channel_id, r.message_ts, COUNT(*) AS cnt FROM reactions r{} GROUP BY r.channel_id, r.message_ts ORDER BY cnt DESC LIMIT ?{}",
        where_sql, idx
    );
    let mut msg_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let msg_limit: u32 = 10;
    msg_params.push(&msg_limit);

    let mut stmt = conn.prepare(&msg_sql)?;
    let rows = stmt.query_map(msg_params.as_slice(), |row| {
        Ok(ReactedMessage {
            channel_id: row.get(0)?,
            message_ts: row.get(1)?,
            reaction_count: row.get(2)?,
        })
    })?;
    let mut most_reacted_messages = Vec::new();
    for row in rows {
        most_reacted_messages.push(row?);
    }

    let report = ReactionsReport {
        channel: opts.channel,
        period: opts.period,
        total_reactions,
        top_emoji,
        most_reacted_messages,
    };

    println!("{}", crate::output::to_json_output(&report));
    Ok(())
}

#[derive(Serialize)]
struct MentionsReport {
    user: Option<String>,
    channel: Option<String>,
    period: Option<String>,
    total_mentions: i64,
    by_channel: Vec<MentionChannelCount>,
    by_mentioner: Vec<MentionerCount>,
}

#[derive(Serialize)]
struct MentionChannelCount {
    channel_id: String,
    mention_count: i64,
}

#[derive(Serialize)]
struct MentionerCount {
    user_id: String,
    mention_count: i64,
}

async fn report_mentions(
    db_path: &std::path::Path,
    opts: crate::cli::ReportMentionsOpts,
) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;

    // Use FTS5 to search for user mentions in message text
    let search_term = if let Some(ref user) = opts.user {
        user.clone()
    } else {
        return Err(crate::error::SlackersError::Store(
            "The --user flag is required for mention reports".to_string(),
        ));
    };

    // Build WHERE conditions for main messages table
    let mut where_clauses = vec!["m.is_deleted = 0".to_string()];
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    // Use LIKE for mention search (FTS5 contentless tables have limitations)
    where_clauses.push(format!("m.text LIKE ?{}", idx));
    param_values.push(Box::new(format!("%{}%", search_term)));
    idx += 1;

    if let Some(ref channel) = opts.channel {
        where_clauses.push(format!("m.channel_id = ?{}", idx));
        param_values.push(Box::new(channel.clone()));
        idx += 1;
    }
    if let Some(ref period) = opts.period {
        let cutoff = period_to_cutoff(period)?;
        where_clauses.push(format!("m.ts >= ?{}", idx));
        param_values.push(Box::new(cutoff));
        idx += 1;
    }

    let where_sql = where_clauses.join(" AND ");
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    // Total mentions
    let total_sql = format!("SELECT COUNT(*) FROM messages m WHERE {}", where_sql);
    let total_mentions: i64 =
        conn.query_row(&total_sql, params_refs.as_slice(), |row| row.get(0))?;

    // By channel
    let channel_sql = format!(
        "SELECT m.channel_id, COUNT(*) AS cnt FROM messages m WHERE {} GROUP BY m.channel_id ORDER BY cnt DESC LIMIT ?{}",
        where_sql, idx
    );
    let mut chan_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let chan_limit: u32 = 20;
    chan_params.push(&chan_limit);

    let mut stmt = conn.prepare(&channel_sql)?;
    let rows = stmt.query_map(chan_params.as_slice(), |row| {
        Ok(MentionChannelCount {
            channel_id: row.get(0)?,
            mention_count: row.get(1)?,
        })
    })?;
    let mut by_channel = Vec::new();
    for row in rows {
        by_channel.push(row?);
    }

    // By mentioner (who mentioned this user)
    let mentioner_sql = format!(
        "SELECT m.user_id, COUNT(*) AS cnt FROM messages m WHERE {} AND m.user_id IS NOT NULL GROUP BY m.user_id ORDER BY cnt DESC LIMIT ?{}",
        where_sql, idx
    );
    let mut mentioner_params = param_values.iter().map(|p| p.as_ref()).collect::<Vec<&dyn rusqlite::types::ToSql>>();
    let mentioner_limit: u32 = 20;
    mentioner_params.push(&mentioner_limit);

    let mut stmt = conn.prepare(&mentioner_sql)?;
    let rows = stmt.query_map(mentioner_params.as_slice(), |row| {
        Ok(MentionerCount {
            user_id: row.get(0)?,
            mention_count: row.get(1)?,
        })
    })?;
    let mut by_mentioner = Vec::new();
    for row in rows {
        by_mentioner.push(row?);
    }

    let report = MentionsReport {
        user: opts.user,
        channel: opts.channel,
        period: opts.period,
        total_mentions,
        by_channel,
        by_mentioner,
    };

    println!("{}", crate::output::to_json_output(&report));
    Ok(())
}
