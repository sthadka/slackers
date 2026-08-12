use crate::error::Result;
use rusqlite::params;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use super::Store;

/// A reaction stored in the local SQLite database.
#[derive(Debug, Clone, Serialize)]
pub struct StoredReaction {
    pub channel_id: String,
    pub message_ts: String,
    pub emoji: String,
    pub user_id: String,
    pub synced_at: i64,
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl Store {
    /// Insert or ignore a reaction (idempotent — the same user+emoji+message is a no-op).
    pub fn upsert_reaction(
        &self,
        channel_id: &str,
        message_ts: &str,
        emoji: &str,
        user_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "INSERT OR REPLACE INTO reactions (channel_id, message_ts, emoji, user_id, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![channel_id, message_ts, emoji, user_id, now_epoch()],
        )?;
        Ok(())
    }

    /// Remove a specific reaction by a specific user.
    pub fn delete_reaction(
        &self,
        channel_id: &str,
        message_ts: &str,
        emoji: &str,
        user_id: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "DELETE FROM reactions WHERE channel_id = ?1 AND message_ts = ?2 AND emoji = ?3 AND user_id = ?4",
            params![channel_id, message_ts, emoji, user_id],
        )?;
        Ok(())
    }

    /// List all reactions on a specific message.
    pub fn list_reactions(
        &self,
        channel_id: &str,
        message_ts: &str,
    ) -> Result<Vec<StoredReaction>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT channel_id, message_ts, emoji, user_id, synced_at
             FROM reactions WHERE channel_id = ?1 AND message_ts = ?2
             ORDER BY emoji, user_id",
        )?;
        let rows = stmt.query_map(params![channel_id, message_ts], |row| {
            Ok(StoredReaction {
                channel_id: row.get(0)?,
                message_ts: row.get(1)?,
                emoji: row.get(2)?,
                user_id: row.get(3)?,
                synced_at: row.get(4)?,
            })
        })?;

        let mut reactions = Vec::new();
        for row in rows {
            reactions.push(row?);
        }
        Ok(reactions)
    }
}

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
    fn test_upsert_and_list_reactions() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        store
            .upsert_reaction("C001", "1700000000.000001", "thumbsup", "U001")
            .unwrap();
        store
            .upsert_reaction("C001", "1700000000.000001", "thumbsup", "U002")
            .unwrap();
        store
            .upsert_reaction("C001", "1700000000.000001", "heart", "U001")
            .unwrap();

        let reactions = store.list_reactions("C001", "1700000000.000001").unwrap();
        assert_eq!(reactions.len(), 3);
    }

    #[test]
    fn test_upsert_reaction_idempotent() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        store
            .upsert_reaction("C001", "1700000000.000001", "thumbsup", "U001")
            .unwrap();
        store
            .upsert_reaction("C001", "1700000000.000001", "thumbsup", "U001")
            .unwrap();

        let reactions = store.list_reactions("C001", "1700000000.000001").unwrap();
        assert_eq!(reactions.len(), 1);
    }

    #[test]
    fn test_delete_reaction() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        store
            .upsert_reaction("C001", "1700000000.000001", "thumbsup", "U001")
            .unwrap();
        store
            .upsert_reaction("C001", "1700000000.000001", "thumbsup", "U002")
            .unwrap();

        store
            .delete_reaction("C001", "1700000000.000001", "thumbsup", "U001")
            .unwrap();

        let reactions = store.list_reactions("C001", "1700000000.000001").unwrap();
        assert_eq!(reactions.len(), 1);
        assert_eq!(reactions[0].user_id, "U002");
    }

    #[test]
    fn test_delete_nonexistent_reaction() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        // Should not error
        store
            .delete_reaction("C001", "1700000000.000001", "thumbsup", "U999")
            .unwrap();
    }

    #[test]
    fn test_list_reactions_empty() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        let reactions = store.list_reactions("C001", "1700000000.000001").unwrap();
        assert!(reactions.is_empty());
    }

    #[test]
    fn test_reaction_fields() {
        let store = Store::open_in_memory().unwrap();
        setup_channel_and_message(&store, "C001", "1700000000.000001");

        store
            .upsert_reaction("C001", "1700000000.000001", "rocket", "U042")
            .unwrap();

        let reactions = store.list_reactions("C001", "1700000000.000001").unwrap();
        assert_eq!(reactions.len(), 1);
        assert_eq!(reactions[0].channel_id, "C001");
        assert_eq!(reactions[0].message_ts, "1700000000.000001");
        assert_eq!(reactions[0].emoji, "rocket");
        assert_eq!(reactions[0].user_id, "U042");
        assert!(reactions[0].synced_at > 0);
    }
}
