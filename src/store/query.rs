use crate::error::{Result, SlackersError};
use crate::store::Store;
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
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
    #[allow(dead_code)]
    pub min_reactions: Option<u32>,
}

/// Parse relative duration strings like "30s", "15m", "8h", "7d", "2w" into a
/// Unix timestamp (as f64 seconds) representing (now - duration).
///
/// Supported unit suffixes (case-insensitive): `s` seconds, `m` minutes,
/// `h` hours, `d` days, `w` weeks. Returns `None` for anything that is not a
/// bare integer followed by one of these units (e.g. raw Slack timestamps like
/// "1700000100.000000"), so callers can treat it as an absolute timestamp.
fn parse_relative_date(s: &str) -> Option<f64> {
    let s = s.trim();
    let (num_str, unit_secs) = match s.chars().last()? {
        's' | 'S' => (&s[..s.len() - 1], 1.0),
        'm' | 'M' => (&s[..s.len() - 1], 60.0),
        'h' | 'H' => (&s[..s.len() - 1], 3600.0),
        'd' | 'D' => (&s[..s.len() - 1], 86400.0),
        'w' | 'W' => (&s[..s.len() - 1], 604800.0),
        _ => return None,
    };
    let n = num_str.parse::<u64>().ok()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    Some(now - (n as f64 * unit_secs))
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

// ============================================================================
// Read-only arbitrary SQL executor
// ============================================================================

/// Produce a copy of `sql` with string/identifier literals and comments replaced
/// by spaces, so structural scans (statement count, leading keyword, LIMIT
/// detection) never trip over SQL embedded in string/identifier bodies.
fn clean_sql(sql: &str) -> String {
    let b = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'\'' || c == b'"' {
            // String literal or quoted identifier; "" / '' escapes the quote.
            let quote = c;
            out.push(' ');
            i += 1;
            while i < b.len() {
                if b[i] == quote {
                    if i + 1 < b.len() && b[i + 1] == quote {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c == b'-' && i + 1 < b.len() && b[i + 1] == b'-' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            out.push(' ');
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

/// The first alphabetic keyword of the cleaned SQL, uppercased.
fn leading_keyword(cleaned: &str) -> String {
    cleaned
        .trim_start()
        .split(|c: char| !c.is_ascii_alphabetic())
        .next()
        .unwrap_or("")
        .to_ascii_uppercase()
}

/// True if the cleaned SQL contains a top-level `LIMIT` keyword (identifier parts
/// like `my_limit` do not count).
fn has_limit_clause(cleaned: &str) -> bool {
    cleaned
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|tok| tok.eq_ignore_ascii_case("limit"))
}

/// Reject anything that is not a single read-only `SELECT`/`WITH` statement.
fn validate_readonly_sql(sql: &str) -> Result<()> {
    let cleaned = clean_sql(sql);
    if cleaned.trim().is_empty() {
        return Err(SlackersError::Store("empty SQL statement".to_string()));
    }
    // Single statement only: a `;` may only be followed by whitespace/comments.
    if let Some(pos) = cleaned.find(';') {
        if !cleaned[pos + 1..].trim().is_empty() {
            return Err(SlackersError::Store(
                "only a single read-only statement is allowed (multiple statements detected)"
                    .to_string(),
            ));
        }
    }
    let kw = leading_keyword(&cleaned);
    if kw != "SELECT" && kw != "WITH" {
        return Err(SlackersError::Store(format!(
            "only read-only SELECT or WITH queries are allowed (got `{kw}`)"
        )));
    }
    Ok(())
}

/// Convert a single SQLite cell to a JSON value by its dynamic type.
fn value_ref_to_json(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => Value::from(n),
        ValueRef::Real(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::String(format!("<blob {} bytes>", b.len())),
    }
}

/// Validate, cap, and execute a read-only query against `conn`, returning one
/// JSON object per row keyed by column name.
fn exec_readonly(conn: &Connection, sql: &str, limit: u32) -> Result<Vec<Value>> {
    validate_readonly_sql(sql)?;
    let cleaned = clean_sql(sql);

    // Apply a safety cap only when the query has no LIMIT of its own. Wrapping in
    // a subquery keeps CTEs intact.
    let needs_cap = !has_limit_clause(&cleaned);
    let final_sql = if needs_cap {
        let inner = sql.trim().trim_end_matches(';').trim();
        format!("SELECT * FROM (\n{inner}\n) LIMIT ?")
    } else {
        sql.to_string()
    };

    let mut stmt = conn
        .prepare(&final_sql)
        .map_err(|e| SlackersError::Store(format!("query failed to prepare: {e}")))?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = stmt
        .column_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let mut rows = if needs_cap {
        stmt.query(rusqlite::params![limit as i64])
    } else {
        stmt.query([])
    }
    .map_err(|e| SlackersError::Store(format!("query failed: {e}")))?;

    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| SlackersError::Store(format!("query failed: {e}")))?
    {
        let mut obj = serde_json::Map::with_capacity(col_count);
        for (i, name) in col_names.iter().enumerate() {
            let cell = row
                .get_ref(i)
                .map_err(|e| SlackersError::Store(format!("query failed: {e}")))?;
            obj.insert(name.clone(), value_ref_to_json(cell));
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

/// Run a read-only SQL query against a dedicated read-only connection to the
/// store at `db_path`. Defense in depth: a `SQLITE_OPEN_READ_ONLY` connection
/// with `query_only = ON` plus statement validation.
pub fn query_sql(db_path: &Path, sql: &str, limit: u32) -> Result<Vec<Value>> {
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| SlackersError::Store(format!("failed to open read-only connection: {e}")))?;
    conn.pragma_update(None, "query_only", "ON")?;
    conn.pragma_update(None, "trusted_schema", "OFF")?;
    exec_readonly(&conn, sql, limit)
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

        // Sub-day units resolve to the correct offset from now.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        for (input, secs) in [("8h", 8.0 * 3600.0), ("15m", 15.0 * 60.0), ("2w", 2.0 * 604800.0), ("30s", 30.0)] {
            let got = parse_relative_date(input).unwrap_or_else(|| panic!("{input} should parse"));
            assert!((got - (now - secs)).abs() < 1.0, "{input} offset wrong");
        }

        // Non-relative inputs return None so callers treat them as raw timestamps.
        assert!(parse_relative_date("notadate").is_none());
        assert!(parse_relative_date("").is_none());
        assert!(parse_relative_date("1700000100.000000").is_none());
        assert!(parse_relative_date("8x").is_none());
    }

    fn setup_sql_store() -> Store {
        let store = Store::open_in_memory().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
                params!["C001", "general", 1000],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO users (id, name, real_name, synced_at) VALUES (?1, ?2, ?3, ?4)",
                params!["U001", "alice", "Alice A", 1000],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO users (id, name, real_name, synced_at) VALUES (?1, ?2, ?3, ?4)",
                params!["U002", "bob", "Bob B", 1000],
            )
            .unwrap();
            let rows: &[(&str, &str, Option<&str>, &str)] = &[
                ("1790727339.341539", "U001", None, "scanner ran out of memory today"),
                ("1790727400.000000", "U002", Some("1790727339.341539"), "reply about memory"),
                ("1790727500.000000", "U001", Some("1790727339.341539"), "another reply"),
                ("1790727600.000000", "U002", None, "unrelated chatter"),
            ];
            for (ts, user, thread, text) in rows {
                conn.execute(
                    "INSERT INTO messages (channel_id, ts, user_id, thread_ts, text, rendered, synced_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params!["C001", ts, user, thread, text, text, 1000i64],
                )
                .unwrap();
            }
        }
        store
    }

    fn message_count(store: &Store) -> i64 {
        let conn = store.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn test_query_sql_join_resolves_names() {
        let store = setup_sql_store();
        let conn = store.conn.lock().unwrap();
        let rows = exec_readonly(
            &conn,
            "SELECT m.ts, COALESCE(u.real_name, u.name) AS author, m.text
             FROM messages m
             LEFT JOIN users u ON u.id = m.user_id
             WHERE m.channel_id = 'C001'
             ORDER BY m.ts",
            1000,
        )
        .unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0]["author"], "Alice A");
        assert_eq!(rows[1]["author"], "Bob B");
    }

    #[test]
    fn test_query_sql_thread_rollup() {
        let store = setup_sql_store();
        let conn = store.conn.lock().unwrap();
        let rows = exec_readonly(
            &conn,
            "SELECT COUNT(*) AS msg_count, COUNT(DISTINCT m.user_id) AS participants
             FROM messages m
             WHERE m.channel_id = 'C001' AND m.is_deleted = 0
               AND COALESCE(m.thread_ts, m.ts) = '1790727339.341539'
             GROUP BY COALESCE(m.thread_ts, m.ts)",
            1000,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["msg_count"], 3);
        assert_eq!(rows[0]["participants"], 2);
    }

    #[test]
    fn test_query_sql_fts_match() {
        let store = setup_sql_store();
        let conn = store.conn.lock().unwrap();
        let rows = exec_readonly(
            &conn,
            "SELECT m.ts, m.text
             FROM messages_fts f
             JOIN messages_rowid_map r ON r.rowid = f.rowid
             JOIN messages m ON m.channel_id = r.channel_id AND m.ts = r.ts
             WHERE messages_fts MATCH 'NEAR(scanner memory, 5)'",
            1000,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["ts"], "1790727339.341539");
    }

    #[test]
    fn test_query_sql_rejects_writes() {
        let store = setup_sql_store();
        let before = message_count(&store);
        let conn = store.conn.lock().unwrap();
        for sql in [
            "DELETE FROM messages",
            "UPDATE messages SET text=''",
            "INSERT INTO messages (channel_id, ts, synced_at) VALUES ('C9','1.0',1)",
            "DROP TABLE messages",
        ] {
            assert!(exec_readonly(&conn, sql, 1000).is_err(), "should reject: {sql}");
        }
        drop(conn);
        assert_eq!(message_count(&store), before);
    }

    #[test]
    fn test_query_sql_rejects_multiple_statements() {
        let store = setup_sql_store();
        let conn = store.conn.lock().unwrap();
        assert!(exec_readonly(&conn, "SELECT 1; SELECT 2", 1000).is_err());
        // A trailing semicolon on a single statement is fine.
        assert!(exec_readonly(&conn, "SELECT 1;", 1000).is_ok());
    }

    #[test]
    fn test_query_sql_applies_limit_cap() {
        let store = setup_sql_store();
        let conn = store.conn.lock().unwrap();
        let rows = exec_readonly(&conn, "SELECT ts FROM messages ORDER BY ts", 2).unwrap();
        assert_eq!(rows.len(), 2);
        // An explicit LIMIT is respected over the cap.
        let rows = exec_readonly(&conn, "SELECT ts FROM messages ORDER BY ts LIMIT 3", 2).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn test_query_sql_readonly_connection_blocks_writes() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let path = dir.path().join("ro.db");
        let store = Store::open(&path).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO channels (id, name, synced_at) VALUES ('C001','general',1000)",
                [],
            )
            .unwrap();

        // Public path returns rows through a dedicated read-only connection.
        let rows = query_sql(&path, "SELECT id FROM channels", 1000).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "C001");

        // Defense in depth: a raw write on the read-only connection fails even if
        // validation were bypassed.
        let ro = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .unwrap();
        ro.pragma_update(None, "query_only", "ON").unwrap();
        assert!(ro.execute("DELETE FROM channels", []).is_err());
    }
}
