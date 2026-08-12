use crate::error::Result;
use rusqlite::params;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use super::Store;

/// A file attachment stored in the local SQLite database.
#[derive(Debug, Clone, Serialize)]
pub struct StoredFile {
    pub id: String,
    pub channel_id: Option<String>,
    pub message_ts: Option<String>,
    pub name: Option<String>,
    pub mimetype: Option<String>,
    pub size_bytes: Option<i64>,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub local_path: Option<String>,
    pub synced_at: i64,
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl Store {
    /// Insert or replace a file record.
    pub fn upsert_file(
        &self,
        id: &str,
        channel_id: Option<&str>,
        message_ts: Option<&str>,
        name: Option<&str>,
        mimetype: Option<&str>,
        size_bytes: Option<i64>,
        url_private: Option<&str>,
        url_private_download: Option<&str>,
        local_path: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "INSERT OR REPLACE INTO files
                (id, channel_id, message_ts, name, mimetype, size_bytes,
                 url_private, url_private_download, local_path, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                channel_id,
                message_ts,
                name,
                mimetype,
                size_bytes,
                url_private,
                url_private_download,
                local_path,
                now_epoch()
            ],
        )?;
        Ok(())
    }

    /// Retrieve a file by its ID.
    pub fn get_file(&self, id: &str) -> Result<Option<StoredFile>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, message_ts, name, mimetype, size_bytes,
                    url_private, url_private_download, local_path, synced_at
             FROM files WHERE id = ?1",
        )?;
        let result = stmt
            .query_row(params![id], |row| {
                Ok(StoredFile {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    message_ts: row.get(2)?,
                    name: row.get(3)?,
                    mimetype: row.get(4)?,
                    size_bytes: row.get(5)?,
                    url_private: row.get(6)?,
                    url_private_download: row.get(7)?,
                    local_path: row.get(8)?,
                    synced_at: row.get(9)?,
                })
            })
            .optional()?;
        Ok(result)
    }

    /// List all files attached to a specific message.
    pub fn list_files_by_message(
        &self,
        channel_id: &str,
        message_ts: &str,
    ) -> Result<Vec<StoredFile>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, message_ts, name, mimetype, size_bytes,
                    url_private, url_private_download, local_path, synced_at
             FROM files WHERE channel_id = ?1 AND message_ts = ?2
             ORDER BY name",
        )?;
        let rows = stmt.query_map(params![channel_id, message_ts], |row| {
            Ok(StoredFile {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                message_ts: row.get(2)?,
                name: row.get(3)?,
                mimetype: row.get(4)?,
                size_bytes: row.get(5)?,
                url_private: row.get(6)?,
                url_private_download: row.get(7)?,
                local_path: row.get(8)?,
                synced_at: row.get(9)?,
            })
        })?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: insert a channel and a message so foreign key constraints are satisfied.
    fn setup_channel_and_message(store: &Store, channel_id: &str, ts: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            params![channel_id, format!("chan-{}", channel_id), now_epoch()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (channel_id, ts, user_id, text, synced_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![channel_id, ts, "U001", "test message", now_epoch()],
        )
        .unwrap();
    }

    #[test]
    fn test_upsert_and_get_file() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        store
            .upsert_file(
                "F001",
                Some("C001"),
                Some("1700000000.000001"),
                Some("report.pdf"),
                Some("application/pdf"),
                Some(1024),
                Some("https://files.slack.com/F001/report.pdf"),
                Some("https://files.slack.com/F001/download/report.pdf"),
                None,
            )
            .unwrap();

        let file = store.get_file("F001").unwrap().expect("file should exist");
        assert_eq!(file.id, "F001");
        assert_eq!(file.channel_id.as_deref(), Some("C001"));
        assert_eq!(file.message_ts.as_deref(), Some("1700000000.000001"));
        assert_eq!(file.name.as_deref(), Some("report.pdf"));
        assert_eq!(file.mimetype.as_deref(), Some("application/pdf"));
        assert_eq!(file.size_bytes, Some(1024));
        assert!(file.url_private.is_some());
        assert!(file.url_private_download.is_some());
        assert!(file.local_path.is_none());
    }

    #[test]
    fn test_upsert_file_replaces_existing() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        store
            .upsert_file("F001", Some("C001"), Some("1700000000.000001"), Some("v1.pdf"), None, None, None, None, None)
            .unwrap();
        store
            .upsert_file("F001", Some("C001"), Some("1700000000.000001"), Some("v2.pdf"), None, None, None, None, None)
            .unwrap();

        let file = store.get_file("F001").unwrap().unwrap();
        assert_eq!(file.name.as_deref(), Some("v2.pdf"));
    }

    #[test]
    fn test_get_file_not_found() {
        let store = Store::open_in_memory().unwrap();
        let result = store.get_file("F999").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_files_by_message() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");
        setup_channel_and_message(&store, "C001b", "1700000000.000002");

        store
            .upsert_file("F001", Some("C001"), Some("1700000000.000001"), Some("a.txt"), None, None, None, None, None)
            .unwrap();
        store
            .upsert_file("F002", Some("C001"), Some("1700000000.000001"), Some("b.txt"), None, None, None, None, None)
            .unwrap();
        // Different message
        store
            .upsert_file("F003", Some("C001b"), Some("1700000000.000002"), Some("c.txt"), None, None, None, None, None)
            .unwrap();

        let files = store.list_files_by_message("C001", "1700000000.000001").unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name.as_deref(), Some("a.txt"));
        assert_eq!(files[1].name.as_deref(), Some("b.txt"));
    }

    #[test]
    fn test_list_files_by_message_empty() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        let files = store.list_files_by_message("C001", "1700000000.000001").unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_file_with_local_path() {
        let store = Store::open_in_memory().unwrap();

        // File not attached to a message (no FK constraint for NULL channel_id/message_ts)
        store
            .upsert_file(
                "F001",
                None,
                None,
                Some("standalone.png"),
                Some("image/png"),
                Some(2048),
                None,
                None,
                Some("/tmp/slackers/files/F001.png"),
            )
            .unwrap();

        let file = store.get_file("F001").unwrap().unwrap();
        assert_eq!(file.local_path.as_deref(), Some("/tmp/slackers/files/F001.png"));
        assert!(file.channel_id.is_none());
        assert!(file.message_ts.is_none());
    }

    #[test]
    fn test_file_minimal_fields() {
        let store = Store::open_in_memory().unwrap();

        store
            .upsert_file("F001", None, None, None, None, None, None, None, None)
            .unwrap();

        let file = store.get_file("F001").unwrap().unwrap();
        assert_eq!(file.id, "F001");
        assert!(file.name.is_none());
        assert!(file.mimetype.is_none());
        assert!(file.size_bytes.is_none());
    }
}
