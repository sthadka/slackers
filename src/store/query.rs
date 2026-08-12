use crate::error::Result;
use crate::store::Store;
use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// Holds all filter parameters for query commands.
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    pub user: Option<String>,
    pub channel: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub text: Option<String>,
    pub group_by: Option<String>,
    pub sort: Option<String>,
    pub limit: u32,
    pub emoji: Option<String>,
    pub min_reactions: Option<u32>,
}

/// Parse relative date strings like "7d", "30d" into Unix timestamps (as f64 seconds).
/// Returns the epoch seconds for (now - Nd).
fn parse_relative_date(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.ends_with('d') || s.ends_with('D') {
        let num_str = &s[..s.len() - 1];
        if let Ok(days) = num_str.parse::<u64>() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            return Some(now - (days as f64 * 86400.0));
        }
    }
    None
}

/// Resolve a time filter value: either a relative date ("7d") or a raw timestamp string.
/// Returns a Slack-style timestamp string (seconds with fractional part).
fn resolve_time_filter(s: &str) -> String {
    if let Some(epoch) = parse_relative_date(s) {
        format!("{:.6}", epoch)
    } else {
        s.to_string()
    }
}

/// Result row for a message query.
#[derive(Debug, Serialize)]
pub struct MessageRow {
    pub channel_id: String,
    pub ts: String,
    pub user_id: Option<String>,
    pub thread_ts: Option<String>,
    pub text: Option<String>,
    pub reply_count: i32,
    pub is_edited: bool,
}

/// Result row for a thread query.
#[derive(Debug, Serialize)]
pub struct ThreadRow {
    pub channel_id: String,
    pub thread_ts: String,
    pub participant_count: i64,
    pub reply_count: i64,
    pub first_reply: Option<String>,
    pub last_reply: Option<String>,
}

/// Result row for a reaction query.
#[derive(Debug, Serialize)]
pub struct ReactionRow {
    pub key: String,
    pub count: i64,
}

/// Result row for a file query.
#[derive(Debug, Serialize)]
pub struct FileRow {
    pub id: String,
    pub channel_id: Option<String>,
    pub name: Option<String>,
    pub mimetype: Option<String>,
    pub size_bytes: Option<i64>,
}

/// Result row for an activity query.
#[derive(Debug, Serialize)]
pub struct ActivityRow {
    pub bucket: String,
    pub message_count: i64,
}

impl Store {
    /// Flexible message query with dynamically built WHERE clauses.
    /// All parameters use `?` placeholders.
    pub fn query_messages(&self, filters: &QueryFilters) -> Result<Vec<Value>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;

        let mut sql = String::from(
            "SELECT channel_id, ts, user_id, thread_ts, text, reply_count, is_edited
             FROM messages WHERE is_deleted = 0",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref channel) = filters.channel {
            sql.push_str(&format!(" AND channel_id = ?{}", idx));
            param_values.push(Box::new(channel.clone()));
            idx += 1;
        }
        if let Some(ref user) = filters.user {
            sql.push_str(&format!(" AND user_id = ?{}", idx));
            param_values.push(Box::new(user.clone()));
            idx += 1;
        }
        if let Some(ref after) = filters.after {
            let ts = resolve_time_filter(after);
            sql.push_str(&format!(" AND ts >= ?{}", idx));
            param_values.push(Box::new(ts));
            idx += 1;
        }
        if let Some(ref before) = filters.before {
            let ts = resolve_time_filter(before);
            sql.push_str(&format!(" AND ts <= ?{}", idx));
            param_values.push(Box::new(ts));
            idx += 1;
        }
        if let Some(ref text) = filters.text {
            sql.push_str(&format!(" AND text LIKE ?{}", idx));
            param_values.push(Box::new(format!("%{}%", text)));
            idx += 1;
        }

        // Sort
        match filters.sort.as_deref() {
            Some("replies") => sql.push_str(" ORDER BY reply_count DESC"),
            _ => sql.push_str(" ORDER BY ts DESC"),
        }

        sql.push_str(&format!(" LIMIT ?{}", idx));
        param_values.push(Box::new(filters.limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(MessageRow {
                channel_id: row.get(0)?,
                ts: row.get(1)?,
                user_id: row.get(2)?,
                thread_ts: row.get(3)?,
                text: row.get(4)?,
                reply_count: row.get(5)?,
                is_edited: row.get::<_, i32>(6)? != 0,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let r = row?;
            results.push(serde_json::to_value(&r).unwrap_or(Value::Null));
        }
        Ok(results)
    }

    /// Query threads: GROUP BY thread_ts, include participant count, reply count, duration.
    pub fn query_threads(&self, filters: &QueryFilters) -> Result<Vec<Value>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;

        let mut sql = String::from(
            "SELECT channel_id, thread_ts,
                    COUNT(DISTINCT user_id) AS participant_count,
                    COUNT(*) AS reply_count,
                    MIN(ts) AS first_reply,
                    MAX(ts) AS last_reply
             FROM messages
             WHERE is_deleted = 0 AND thread_ts IS NOT NULL",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref channel) = filters.channel {
            sql.push_str(&format!(" AND channel_id = ?{}", idx));
            param_values.push(Box::new(channel.clone()));
            idx += 1;
        }
        if let Some(ref user) = filters.user {
            sql.push_str(&format!(" AND user_id = ?{}", idx));
            param_values.push(Box::new(user.clone()));
            idx += 1;
        }
        if let Some(ref after) = filters.after {
            let ts = resolve_time_filter(after);
            sql.push_str(&format!(" AND ts >= ?{}", idx));
            param_values.push(Box::new(ts));
            idx += 1;
        }
        if let Some(ref before) = filters.before {
            let ts = resolve_time_filter(before);
            sql.push_str(&format!(" AND ts <= ?{}", idx));
            param_values.push(Box::new(ts));
            idx += 1;
        }

        sql.push_str(" GROUP BY channel_id, thread_ts");

        match filters.sort.as_deref() {
            Some("participants") => sql.push_str(" ORDER BY participant_count DESC"),
            Some("duration") => sql.push_str(" ORDER BY (CAST(MAX(ts) AS REAL) - CAST(MIN(ts) AS REAL)) DESC"),
            _ => sql.push_str(" ORDER BY reply_count DESC"),
        }

        sql.push_str(&format!(" LIMIT ?{}", idx));
        param_values.push(Box::new(filters.limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(ThreadRow {
                channel_id: row.get(0)?,
                thread_ts: row.get(1)?,
                participant_count: row.get(2)?,
                reply_count: row.get(3)?,
                first_reply: row.get(4)?,
                last_reply: row.get(5)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let r = row?;
            results.push(serde_json::to_value(&r).unwrap_or(Value::Null));
        }
        Ok(results)
    }

    /// Query reactions: group by emoji or user, COUNT aggregation.
    pub fn query_reactions(&self, filters: &QueryFilters) -> Result<Vec<Value>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;

        let group_col = match filters.group_by.as_deref() {
            Some("user") => "user_id",
            _ => "emoji",
        };

        let mut where_clauses: Vec<String> = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref channel) = filters.channel {
            where_clauses.push(format!("channel_id = ?{}", idx));
            param_values.push(Box::new(channel.clone()));
            idx += 1;
        }
        if let Some(ref emoji) = filters.emoji {
            where_clauses.push(format!("emoji = ?{}", idx));
            param_values.push(Box::new(emoji.clone()));
            idx += 1;
        }
        if let Some(ref user) = filters.user {
            where_clauses.push(format!("user_id = ?{}", idx));
            param_values.push(Box::new(user.clone()));
            idx += 1;
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT {col} AS key, COUNT(*) AS count FROM reactions{where_sql} GROUP BY {col} ORDER BY count DESC LIMIT ?{idx}",
            col = group_col,
            where_sql = where_sql,
            idx = idx,
        );
        param_values.push(Box::new(filters.limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(ReactionRow {
                key: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let r = row?;
            results.push(serde_json::to_value(&r).unwrap_or(Value::Null));
        }
        Ok(results)
    }

    /// Query files: filter by type/channel, sort by size.
    pub fn query_files(&self, filters: &QueryFilters) -> Result<Vec<Value>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;

        let mut sql = String::from(
            "SELECT id, channel_id, name, mimetype, size_bytes FROM files WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref channel) = filters.channel {
            sql.push_str(&format!(" AND channel_id = ?{}", idx));
            param_values.push(Box::new(channel.clone()));
            idx += 1;
        }
        if let Some(ref text) = filters.text {
            // Filter by mimetype or name
            sql.push_str(&format!(" AND (mimetype LIKE ?{idx} OR name LIKE ?{idx})", idx = idx));
            param_values.push(Box::new(format!("%{}%", text)));
            idx += 1;
        }

        match filters.sort.as_deref() {
            Some("name") => sql.push_str(" ORDER BY name ASC"),
            _ => sql.push_str(" ORDER BY size_bytes DESC"),
        }

        sql.push_str(&format!(" LIMIT ?{}", idx));
        param_values.push(Box::new(filters.limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(FileRow {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                name: row.get(2)?,
                mimetype: row.get(3)?,
                size_bytes: row.get(4)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let r = row?;
            results.push(serde_json::to_value(&r).unwrap_or(Value::Null));
        }
        Ok(results)
    }

    /// Query activity: GROUP BY channel+date, COUNT messages per bucket.
    pub fn query_activity(&self, filters: &QueryFilters) -> Result<Vec<Value>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;

        // Group by date (YYYY-MM-DD derived from the ts column which is a Unix timestamp string)
        let mut sql = String::from(
            "SELECT DATE(CAST(ts AS REAL), 'unixepoch') AS day,
                    COUNT(*) AS message_count
             FROM messages WHERE is_deleted = 0",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref channel) = filters.channel {
            sql.push_str(&format!(" AND channel_id = ?{}", idx));
            param_values.push(Box::new(channel.clone()));
            idx += 1;
        }
        if let Some(ref user) = filters.user {
            sql.push_str(&format!(" AND user_id = ?{}", idx));
            param_values.push(Box::new(user.clone()));
            idx += 1;
        }
        if let Some(ref after) = filters.after {
            let ts = resolve_time_filter(after);
            sql.push_str(&format!(" AND ts >= ?{}", idx));
            param_values.push(Box::new(ts));
            idx += 1;
        }
        if let Some(ref before) = filters.before {
            let ts = resolve_time_filter(before);
            sql.push_str(&format!(" AND ts <= ?{}", idx));
            param_values.push(Box::new(ts));
            idx += 1;
        }

        sql.push_str(" GROUP BY day ORDER BY day DESC");

        sql.push_str(&format!(" LIMIT ?{}", idx));
        param_values.push(Box::new(filters.limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(ActivityRow {
                bucket: row.get(0)?,
                message_count: row.get(1)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let r = row?;
            results.push(serde_json::to_value(&r).unwrap_or(Value::Null));
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn setup_store() -> Store {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            params!["C001", "general", 1000],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            params!["C002", "random", 1000],
        )
        .unwrap();
        // Insert messages
        for i in 1..=5 {
            conn.execute(
                "INSERT INTO messages (channel_id, ts, user_id, thread_ts, text, rendered, reply_count, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "C001",
                    format!("{}.000000", 1700000000 + i * 100),
                    if i % 2 == 0 { "U002" } else { "U001" },
                    if i > 3 { Some(format!("{}.000000", 1700000000 + 100)) } else { None::<String> },
                    format!("message {}", i),
                    format!("message {}", i),
                    if i == 1 { 2 } else { 0 },
                    1000i64,
                ],
            )
            .unwrap();
        }
        // Insert reactions
        conn.execute(
            "INSERT INTO reactions (channel_id, message_ts, emoji, user_id, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["C001", "1700000100.000000", "thumbsup", "U002", 1000],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO reactions (channel_id, message_ts, emoji, user_id, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["C001", "1700000100.000000", "thumbsup", "U003", 1000],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO reactions (channel_id, message_ts, emoji, user_id, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["C001", "1700000200.000000", "heart", "U001", 1000],
        )
        .unwrap();
        // Insert files
        conn.execute(
            "INSERT INTO files (id, channel_id, message_ts, name, mimetype, size_bytes, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params!["F001", "C001", "1700000100.000000", "report.pdf", "application/pdf", 1024, 1000],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files (id, channel_id, message_ts, name, mimetype, size_bytes, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params!["F002", "C001", "1700000200.000000", "image.png", "image/png", 2048, 1000],
        )
        .unwrap();
        drop(conn);
        store
    }

    #[test]
    fn test_query_messages_no_filters() {
        let store = setup_store();
        let filters = QueryFilters {
            limit: 50,
            ..Default::default()
        };
        let results = store.query_messages(&filters).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_query_messages_by_channel() {
        let store = setup_store();
        let filters = QueryFilters {
            channel: Some("C001".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results = store.query_messages(&filters).unwrap();
        assert_eq!(results.len(), 5);

        let filters2 = QueryFilters {
            channel: Some("C002".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results2 = store.query_messages(&filters2).unwrap();
        assert_eq!(results2.len(), 0);
    }

    #[test]
    fn test_query_messages_by_user() {
        let store = setup_store();
        let filters = QueryFilters {
            user: Some("U001".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results = store.query_messages(&filters).unwrap();
        assert_eq!(results.len(), 3); // messages 1, 3, 5
    }

    #[test]
    fn test_query_messages_with_text_filter() {
        let store = setup_store();
        let filters = QueryFilters {
            text: Some("message 3".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results = store.query_messages(&filters).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_messages_with_limit() {
        let store = setup_store();
        let filters = QueryFilters {
            limit: 2,
            ..Default::default()
        };
        let results = store.query_messages(&filters).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_threads() {
        let store = setup_store();
        let filters = QueryFilters {
            channel: Some("C001".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results = store.query_threads(&filters).unwrap();
        // Messages 4 and 5 have thread_ts set
        assert!(!results.is_empty());
    }

    #[test]
    fn test_query_reactions_by_emoji() {
        let store = setup_store();
        let filters = QueryFilters {
            limit: 50,
            ..Default::default()
        };
        let results = store.query_reactions(&filters).unwrap();
        assert_eq!(results.len(), 2); // thumbsup (2) and heart (1)
        // First should be thumbsup with count 2
        assert_eq!(results[0]["key"], "thumbsup");
        assert_eq!(results[0]["count"], 2);
    }

    #[test]
    fn test_query_reactions_by_user() {
        let store = setup_store();
        let filters = QueryFilters {
            group_by: Some("user".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results = store.query_reactions(&filters).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_query_files() {
        let store = setup_store();
        let filters = QueryFilters {
            channel: Some("C001".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results = store.query_files(&filters).unwrap();
        assert_eq!(results.len(), 2);
        // Sorted by size_bytes DESC, so image.png (2048) first
        assert_eq!(results[0]["name"], "image.png");
    }

    #[test]
    fn test_query_activity() {
        let store = setup_store();
        let filters = QueryFilters {
            channel: Some("C001".to_string()),
            limit: 50,
            ..Default::default()
        };
        let results = store.query_activity(&filters).unwrap();
        assert!(!results.is_empty());
        // All messages are on the same day
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_parse_relative_date() {
        let result = parse_relative_date("7d");
        assert!(result.is_some());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let expected_approx = now - 7.0 * 86400.0;
        assert!((result.unwrap() - expected_approx).abs() < 1.0);

        assert!(parse_relative_date("30d").is_some());
        assert!(parse_relative_date("notadate").is_none());
        assert!(parse_relative_date("").is_none());
    }
}
