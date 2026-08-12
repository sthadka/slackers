pub mod channels;
pub mod files;
pub mod fts;
pub mod messages;
pub mod query;
pub mod reactions;
mod schema;
pub mod subscriptions;
pub mod users;

use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Open (or create) a store database at the given path.
    /// Sets WAL mode, busy_timeout, foreign_keys, and runs migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::init_connection(&conn)?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory store (for testing).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_connection(&conn)?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Configure connection pragmas.
    fn init_connection(conn: &Connection) -> Result<()> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(())
    }

    /// Run schema migrations based on user_version pragma.
    fn migrate(conn: &Connection) -> Result<()> {
        let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < 1 {
            conn.execute_batch(schema::SCHEMA_V1)?;
            conn.pragma_update(None, "user_version", 1)?;
        }
        // Future: if version < 2 { ... }
        Ok(())
    }

    /// Get the current schema version.
    #[allow(dead_code)]
    pub fn schema_version(&self) -> Result<u32> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_store_open_creates_db_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        assert!(!path.exists());

        let _store = Store::open(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_store_open_sets_wal_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal-test.db");
        let store = Store::open(&path).unwrap();

        let conn = store.conn.lock().unwrap();
        let mode: String =
            conn.pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn test_store_open_sets_user_version() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);
    }

    #[test]
    fn test_store_creates_all_tables() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();

        let expected_tables = [
            "workspace",
            "channels",
            "users",
            "messages",
            "reactions",
            "files",
            "channel_members",
            "sync_state",
            "subscriptions",
            "saved_items",
            "pins",
            "messages_rowid_map",
            "messages_fts",
        ];

        for table in &expected_tables {
            let exists: bool = conn
                .prepare(&format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1"
                ))
                .unwrap()
                .query_row([table], |r| r.get::<_, i64>(0))
                .map(|count| count > 0)
                .unwrap();
            assert!(exists, "table '{}' should exist", table);
        }
    }

    #[test]
    fn test_store_creates_indexes() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();

        let expected_indexes = [
            "idx_channels_name",
            "idx_users_name",
            "idx_messages_thread",
            "idx_messages_user",
            "idx_messages_time",
            "idx_reactions_emoji",
        ];

        for index in &expected_indexes {
            let exists: bool = conn
                .prepare(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                )
                .unwrap()
                .query_row([index], |r| r.get::<_, i64>(0))
                .map(|count| count > 0)
                .unwrap();
            assert!(exists, "index '{}' should exist", index);
        }
    }

    #[test]
    fn test_store_creates_triggers() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();

        let expected_triggers = ["messages_ai", "messages_au", "messages_ad"];

        for trigger in &expected_triggers {
            let exists: bool = conn
                .prepare(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
                )
                .unwrap()
                .query_row([trigger], |r| r.get::<_, i64>(0))
                .map(|count| count > 0)
                .unwrap();
            assert!(exists, "trigger '{}' should exist", trigger);
        }
    }

    #[test]
    fn test_store_migration_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("idempotent-test.db");

        // Open once to create schema
        {
            let _store = Store::open(&path).unwrap();
        }

        // Open again -- should not fail
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);
    }

    #[test]
    fn test_store_in_memory() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);
    }

    #[test]
    fn test_store_fts_trigger_insert() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();

        // Insert a channel first (foreign key)
        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["C001", "general", 1000],
        )
        .unwrap();

        // Insert a message
        conn.execute(
            "INSERT INTO messages (channel_id, ts, user_id, text, rendered, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["C001", "1234567890.000001", "U001", "hello world", "hello world", 1000],
        )
        .unwrap();

        // Verify FTS trigger populated messages_rowid_map
        let map_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages_rowid_map", [], |r| r.get(0))
            .unwrap();
        assert_eq!(map_count, 1);

        // Verify FTS search works
        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages_fts WHERE messages_fts MATCH ?1",
                ["hello"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 1);
    }

    #[test]
    fn test_store_fts_trigger_delete() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["C001", "general", 1000],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO messages (channel_id, ts, user_id, text, rendered, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["C001", "1234567890.000001", "U001", "delete me", "delete me", 1000],
        )
        .unwrap();

        // Delete the message
        conn.execute(
            "DELETE FROM messages WHERE channel_id = ?1 AND ts = ?2",
            rusqlite::params!["C001", "1234567890.000001"],
        )
        .unwrap();

        // Verify FTS entry and rowid map entry are gone
        let map_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages_rowid_map", [], |r| r.get(0))
            .unwrap();
        assert_eq!(map_count, 0);

        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages_fts WHERE messages_fts MATCH ?1",
                ["delete"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 0);
    }

    #[test]
    fn test_store_foreign_keys_enabled() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();

        let fk: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_store_busy_timeout() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();

        let timeout: i64 = conn
            .pragma_query_value(None, "busy_timeout", |r| r.get(0))
            .unwrap();
        assert_eq!(timeout, 5000);
    }
}
