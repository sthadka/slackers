use crate::error::Result;
use rusqlite::params;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use super::Store;

/// A message stored in the local SQLite database.
#[derive(Debug, Clone, Serialize)]
pub struct StoredMessage {
    pub channel_id: String,
    pub ts: String,
    pub user_id: Option<String>,
    pub thread_ts: Option<String>,
    pub text: Option<String>,
    pub rendered: Option<String>,
    pub subtype: Option<String>,
    pub reply_count: i32,
    pub is_edited: bool,
    pub is_deleted: bool,
    pub raw_json: Option<String>,
    pub synced_at: i64,
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl Store {
    /// Insert or replace a message. FTS triggers handle index maintenance.
    pub fn upsert_message(
        &self,
        channel_id: &str,
        ts: &str,
        user_id: Option<&str>,
        thread_ts: Option<&str>,
        text: Option<&str>,
        rendered: Option<&str>,
        subtype: Option<&str>,
        reply_count: i32,
        raw_json: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "INSERT OR REPLACE INTO messages
                (channel_id, ts, user_id, thread_ts, text, rendered, subtype, reply_count, raw_json, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![channel_id, ts, user_id, thread_ts, text, rendered, subtype, reply_count, raw_json, now_epoch()],
        )?;
        Ok(())
    }

    /// Retrieve a single message by its composite key (channel_id, ts).
    pub fn get_message(&self, channel_id: &str, ts: &str) -> Result<Option<StoredMessage>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT channel_id, ts, user_id, thread_ts, text, rendered, subtype,
                    reply_count, is_edited, is_deleted, raw_json, synced_at
             FROM messages WHERE channel_id = ?1 AND ts = ?2",
        )?;
        let result = stmt
            .query_row(params![channel_id, ts], |row| {
                Ok(StoredMessage {
                    channel_id: row.get(0)?,
                    ts: row.get(1)?,
                    user_id: row.get(2)?,
                    thread_ts: row.get(3)?,
                    text: row.get(4)?,
                    rendered: row.get(5)?,
                    subtype: row.get(6)?,
                    reply_count: row.get(7)?,
                    is_edited: row.get::<_, i32>(8)? != 0,
                    is_deleted: row.get::<_, i32>(9)? != 0,
                    raw_json: row.get(10)?,
                    synced_at: row.get(11)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    /// List messages in a channel, optionally bounded by timestamp range, limited to `limit` rows.
    /// Ordered by ts ascending.
    pub fn list_messages(
        &self,
        channel_id: &str,
        oldest: Option<&str>,
        newest: Option<&str>,
        limit: u32,
    ) -> Result<Vec<StoredMessage>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;

        let mut sql = String::from(
            "SELECT channel_id, ts, user_id, thread_ts, text, rendered, subtype,
                    reply_count, is_edited, is_deleted, raw_json, synced_at
             FROM messages WHERE channel_id = ?1 AND is_deleted = 0",
        );
        // We'll use dynamic params. Params: ?1 = channel_id, then conditionally ?2/?3/?4.
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(channel_id.to_string()));

        let mut param_idx = 2;
        if let Some(oldest_ts) = oldest {
            sql.push_str(&format!(" AND ts >= ?{}", param_idx));
            param_values.push(Box::new(oldest_ts.to_string()));
            param_idx += 1;
        }
        if let Some(newest_ts) = newest {
            sql.push_str(&format!(" AND ts <= ?{}", param_idx));
            param_values.push(Box::new(newest_ts.to_string()));
            param_idx += 1;
        }
        sql.push_str(&format!(" ORDER BY ts ASC LIMIT ?{}", param_idx));
        param_values.push(Box::new(limit));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(StoredMessage {
                channel_id: row.get(0)?,
                ts: row.get(1)?,
                user_id: row.get(2)?,
                thread_ts: row.get(3)?,
                text: row.get(4)?,
                rendered: row.get(5)?,
                subtype: row.get(6)?,
                reply_count: row.get(7)?,
                is_edited: row.get::<_, i32>(8)? != 0,
                is_deleted: row.get::<_, i32>(9)? != 0,
                raw_json: row.get(10)?,
                synced_at: row.get(11)?,
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    /// List all messages in a thread (messages whose thread_ts matches the given value),
    /// including the parent message itself. Ordered by ts ascending.
    pub fn list_thread(
        &self,
        channel_id: &str,
        thread_ts: &str,
    ) -> Result<Vec<StoredMessage>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT channel_id, ts, user_id, thread_ts, text, rendered, subtype,
                    reply_count, is_edited, is_deleted, raw_json, synced_at
             FROM messages
             WHERE channel_id = ?1 AND (ts = ?2 OR thread_ts = ?2) AND is_deleted = 0
             ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(params![channel_id, thread_ts], |row| {
            Ok(StoredMessage {
                channel_id: row.get(0)?,
                ts: row.get(1)?,
                user_id: row.get(2)?,
                thread_ts: row.get(3)?,
                text: row.get(4)?,
                rendered: row.get(5)?,
                subtype: row.get(6)?,
                reply_count: row.get(7)?,
                is_edited: row.get::<_, i32>(8)? != 0,
                is_deleted: row.get::<_, i32>(9)? != 0,
                raw_json: row.get(10)?,
                synced_at: row.get(11)?,
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    /// Hard-delete a message (removes the row entirely). FTS triggers clean up the index.
    #[allow(dead_code)]
    pub fn delete_message(&self, channel_id: &str, ts: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "DELETE FROM messages WHERE channel_id = ?1 AND ts = ?2",
            params![channel_id, ts],
        )?;
        Ok(())
    }

    /// Soft-delete a message (sets is_deleted = 1, preserving the row).
    pub fn soft_delete_message(&self, channel_id: &str, ts: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "UPDATE messages SET is_deleted = 1, synced_at = ?3 WHERE channel_id = ?1 AND ts = ?2",
            params![channel_id, ts, now_epoch()],
        )?;
        Ok(())
    }

    /// Mark a message as edited, updating its text and rendered content.
    pub fn mark_edited(
        &self,
        channel_id: &str,
        ts: &str,
        new_text: &str,
        rendered: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "UPDATE messages SET text = ?3, rendered = ?4, is_edited = 1, synced_at = ?5
             WHERE channel_id = ?1 AND ts = ?2",
            params![channel_id, ts, new_text, rendered, now_epoch()],
        )?;
        Ok(())
    }

    /// Count non-deleted messages in a channel.
    #[allow(dead_code)]
    pub fn message_count(&self, channel_id: &str) -> Result<u64> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE channel_id = ?1 AND is_deleted = 0",
            params![channel_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }
}

/// Allow `query_row` to return `None` instead of an error when no rows match.
use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: insert a channel so foreign key constraints are satisfied.
    fn insert_channel(store: &Store, id: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            params![id, format!("chan-{}", id), now_epoch()],
        )
        .unwrap();
    }

    #[test]
    fn test_upsert_and_get_message() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message(
                "C001",
                "1700000000.000001",
                Some("U001"),
                None,
                Some("hello world"),
                Some("hello world"),
                None,
                0,
                None,
            )
            .unwrap();

        let msg = store
            .get_message("C001", "1700000000.000001")
            .unwrap()
            .expect("message should exist");
        assert_eq!(msg.channel_id, "C001");
        assert_eq!(msg.ts, "1700000000.000001");
        assert_eq!(msg.user_id.as_deref(), Some("U001"));
        assert_eq!(msg.text.as_deref(), Some("hello world"));
        assert!(!msg.is_edited);
        assert!(!msg.is_deleted);
    }

    #[test]
    fn test_upsert_replaces_existing() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("v1"), None, None, 0, None)
            .unwrap();
        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("v2"), None, None, 0, None)
            .unwrap();

        let msg = store.get_message("C001", "1700000000.000001").unwrap().unwrap();
        assert_eq!(msg.text.as_deref(), Some("v2"));
    }

    #[test]
    fn test_get_message_not_found() {
        let store = Store::open_in_memory().unwrap();
        let result = store.get_message("C999", "9999999999.000001").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_messages_basic() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        for i in 1..=5 {
            store
                .upsert_message(
                    "C001",
                    &format!("1700000000.00000{}", i),
                    Some("U001"),
                    None,
                    Some(&format!("msg {}", i)),
                    None,
                    None,
                    0,
                    None,
                )
                .unwrap();
        }

        let messages = store.list_messages("C001", None, None, 100).unwrap();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].ts, "1700000000.000001");
        assert_eq!(messages[4].ts, "1700000000.000005");
    }

    #[test]
    fn test_list_messages_with_bounds() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        for i in 1..=5 {
            store
                .upsert_message(
                    "C001",
                    &format!("1700000000.00000{}", i),
                    Some("U001"),
                    None,
                    Some(&format!("msg {}", i)),
                    None,
                    None,
                    0,
                    None,
                )
                .unwrap();
        }

        let messages = store
            .list_messages(
                "C001",
                Some("1700000000.000002"),
                Some("1700000000.000004"),
                100,
            )
            .unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].ts, "1700000000.000002");
        assert_eq!(messages[2].ts, "1700000000.000004");
    }

    #[test]
    fn test_list_messages_with_limit() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        for i in 1..=10 {
            store
                .upsert_message(
                    "C001",
                    &format!("1700000000.0000{:02}", i),
                    Some("U001"),
                    None,
                    Some(&format!("msg {}", i)),
                    None,
                    None,
                    0,
                    None,
                )
                .unwrap();
        }

        let messages = store.list_messages("C001", None, None, 3).unwrap();
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_list_messages_excludes_deleted() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("keep"), None, None, 0, None)
            .unwrap();
        store
            .upsert_message("C001", "1700000000.000002", Some("U001"), None, Some("delete"), None, None, 0, None)
            .unwrap();

        store.soft_delete_message("C001", "1700000000.000002").unwrap();

        let messages = store.list_messages("C001", None, None, 100).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text.as_deref(), Some("keep"));
    }

    #[test]
    fn test_list_thread() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        // Parent message
        store
            .upsert_message(
                "C001",
                "1700000000.000001",
                Some("U001"),
                None,
                Some("parent"),
                None,
                None,
                2,
                None,
            )
            .unwrap();
        // Thread replies
        store
            .upsert_message(
                "C001",
                "1700000000.000002",
                Some("U002"),
                Some("1700000000.000001"),
                Some("reply 1"),
                None,
                None,
                0,
                None,
            )
            .unwrap();
        store
            .upsert_message(
                "C001",
                "1700000000.000003",
                Some("U003"),
                Some("1700000000.000001"),
                Some("reply 2"),
                None,
                None,
                0,
                None,
            )
            .unwrap();
        // Unrelated message
        store
            .upsert_message(
                "C001",
                "1700000000.000004",
                Some("U001"),
                None,
                Some("other"),
                None,
                None,
                0,
                None,
            )
            .unwrap();

        let thread = store.list_thread("C001", "1700000000.000001").unwrap();
        assert_eq!(thread.len(), 3);
        assert_eq!(thread[0].text.as_deref(), Some("parent"));
        assert_eq!(thread[1].text.as_deref(), Some("reply 1"));
        assert_eq!(thread[2].text.as_deref(), Some("reply 2"));
    }

    #[test]
    fn test_delete_message() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("bye"), None, None, 0, None)
            .unwrap();

        store.delete_message("C001", "1700000000.000001").unwrap();

        let result = store.get_message("C001", "1700000000.000001").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_soft_delete_message() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("soft"), None, None, 0, None)
            .unwrap();

        store.soft_delete_message("C001", "1700000000.000001").unwrap();

        let msg = store.get_message("C001", "1700000000.000001").unwrap().unwrap();
        assert!(msg.is_deleted);
    }

    #[test]
    fn test_mark_edited() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("original"), Some("original"), None, 0, None)
            .unwrap();

        store
            .mark_edited("C001", "1700000000.000001", "edited text", Some("edited rendered"))
            .unwrap();

        let msg = store.get_message("C001", "1700000000.000001").unwrap().unwrap();
        assert!(msg.is_edited);
        assert_eq!(msg.text.as_deref(), Some("edited text"));
        assert_eq!(msg.rendered.as_deref(), Some("edited rendered"));
    }

    #[test]
    fn test_message_count() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        assert_eq!(store.message_count("C001").unwrap(), 0);

        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("a"), None, None, 0, None)
            .unwrap();
        store
            .upsert_message("C001", "1700000000.000002", Some("U001"), None, Some("b"), None, None, 0, None)
            .unwrap();

        assert_eq!(store.message_count("C001").unwrap(), 2);

        // Soft-deleted messages should not be counted
        store.soft_delete_message("C001", "1700000000.000002").unwrap();
        assert_eq!(store.message_count("C001").unwrap(), 1);
    }

    #[test]
    fn test_message_with_thread_ts() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message(
                "C001",
                "1700000000.000002",
                Some("U001"),
                Some("1700000000.000001"),
                Some("threaded reply"),
                None,
                None,
                0,
                None,
            )
            .unwrap();

        let msg = store.get_message("C001", "1700000000.000002").unwrap().unwrap();
        assert_eq!(msg.thread_ts.as_deref(), Some("1700000000.000001"));
    }

    #[test]
    fn test_message_with_raw_json() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        let raw = r#"{"type":"message","text":"hello"}"#;
        store
            .upsert_message("C001", "1700000000.000001", Some("U001"), None, Some("hello"), None, None, 0, Some(raw))
            .unwrap();

        let msg = store.get_message("C001", "1700000000.000001").unwrap().unwrap();
        assert_eq!(msg.raw_json.as_deref(), Some(raw));
    }

    #[test]
    fn test_message_with_subtype() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .upsert_message(
                "C001",
                "1700000000.000001",
                None,
                None,
                Some("joined #general"),
                None,
                Some("channel_join"),
                0,
                None,
            )
            .unwrap();

        let msg = store.get_message("C001", "1700000000.000001").unwrap().unwrap();
        assert_eq!(msg.subtype.as_deref(), Some("channel_join"));
    }
}
