use crate::error::Result;
use crate::store::Store;
use rusqlite::params;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// A channel subscription — which channels the user wants to keep synced.
#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    pub channel_id: String,
    pub channel_name: Option<String>,
    pub subscribed_at: i64,
    pub retention_days: Option<u32>,
    pub sync_threads: bool,
    pub sync_members: bool,
}

/// Per-channel sync state — tracks what has been synced.
#[derive(Debug, Clone, Serialize)]
pub struct SyncState {
    pub channel_id: String,
    pub oldest_ts: Option<String>,
    pub newest_ts: Option<String>,
    pub is_complete: bool,
    pub last_sync: i64,
    pub cursor: Option<String>,
}

fn epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

impl Store {
    /// Add a subscription for a channel. If the channel is already subscribed,
    /// this updates the existing subscription (upsert).
    pub fn add_subscription(
        &self,
        channel_id: &str,
        channel_name: Option<&str>,
        retention_days: Option<u32>,
        sync_threads: bool,
        sync_members: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "INSERT INTO subscriptions (channel_id, channel_name, subscribed_at, retention_days, sync_threads, sync_members)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(channel_id) DO UPDATE SET
                 channel_name = excluded.channel_name,
                 retention_days = excluded.retention_days,
                 sync_threads = excluded.sync_threads,
                 sync_members = excluded.sync_members",
            params![
                channel_id,
                channel_name,
                epoch_ms(),
                retention_days,
                sync_threads as i32,
                sync_members as i32,
            ],
        )?;
        Ok(())
    }

    /// Remove a subscription for a channel.
    pub fn remove_subscription(&self, channel_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "DELETE FROM subscriptions WHERE channel_id = ?1",
            params![channel_id],
        )?;
        Ok(())
    }

    /// List all subscriptions.
    pub fn list_subscriptions(&self) -> Result<Vec<Subscription>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT channel_id, channel_name, subscribed_at, retention_days, sync_threads, sync_members
             FROM subscriptions
             ORDER BY subscribed_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Subscription {
                channel_id: row.get(0)?,
                channel_name: row.get(1)?,
                subscribed_at: row.get(2)?,
                retention_days: row.get(3)?,
                sync_threads: row.get::<_, i32>(4)? != 0,
                sync_members: row.get::<_, i32>(5)? != 0,
            })
        })?;
        let mut subs = Vec::new();
        for row in rows {
            subs.push(row?);
        }
        Ok(subs)
    }

    /// List only the channel IDs of all subscriptions.
    pub fn list_subscription_channel_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT channel_id FROM subscriptions ORDER BY subscribed_at",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    /// Check whether a channel is subscribed.
    pub fn is_subscribed(&self, channel_id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM subscriptions WHERE channel_id = ?1",
            params![channel_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get the sync state for a channel.
    pub fn get_sync_state(&self, channel_id: &str) -> Result<Option<SyncState>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(
            "SELECT channel_id, oldest_ts, newest_ts, is_complete, last_sync, cursor
             FROM sync_state
             WHERE channel_id = ?1",
        )?;
        let result = stmt.query_row(params![channel_id], |row| {
            Ok(SyncState {
                channel_id: row.get(0)?,
                oldest_ts: row.get(1)?,
                newest_ts: row.get(2)?,
                is_complete: row.get::<_, i32>(3)? != 0,
                last_sync: row.get(4)?,
                cursor: row.get(5)?,
            })
        });
        match result {
            Ok(state) => Ok(Some(state)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update (upsert) the sync state for a channel.
    pub fn update_sync_state(
        &self,
        channel_id: &str,
        oldest_ts: Option<&str>,
        newest_ts: Option<&str>,
        cursor: Option<&str>,
        is_complete: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "INSERT INTO sync_state (channel_id, oldest_ts, newest_ts, cursor, is_complete, last_sync)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(channel_id) DO UPDATE SET
                 oldest_ts = COALESCE(?2, sync_state.oldest_ts),
                 newest_ts = COALESCE(?3, sync_state.newest_ts),
                 cursor = ?4,
                 is_complete = ?5,
                 last_sync = ?6",
            params![
                channel_id,
                oldest_ts,
                newest_ts,
                cursor,
                is_complete as i32,
                epoch_ms(),
            ],
        )?;
        Ok(())
    }

    /// Set just the cursor for resumable backfill, updating last_sync timestamp.
    pub fn set_sync_cursor(&self, channel_id: &str, cursor: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let rows = conn.execute(
            "UPDATE sync_state SET cursor = ?1, last_sync = ?2 WHERE channel_id = ?3",
            params![cursor, epoch_ms(), channel_id],
        )?;
        if rows == 0 {
            // No existing sync_state row — create one with just the cursor
            conn.execute(
                "INSERT INTO sync_state (channel_id, cursor, is_complete, last_sync)
                 VALUES (?1, ?2, 0, ?3)",
                params![channel_id, cursor, epoch_ms()],
            )?;
        }
        Ok(())
    }

    /// Delete messages older than `before_ts` for the given channel.
    /// Returns the number of messages deleted.
    pub fn delete_old_messages(&self, channel_id: &str, before_ts: &str) -> Result<u64> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let deleted = conn.execute(
            "DELETE FROM messages WHERE channel_id = ?1 AND ts < ?2",
            params![channel_id, before_ts],
        )?;
        Ok(deleted as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: insert a channel into the channels table (needed for FK constraints).
    fn insert_channel(store: &Store, channel_id: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO channels (id, name, synced_at) VALUES (?1, ?2, ?3)",
            params![channel_id, format!("chan-{}", channel_id), epoch_ms()],
        )
        .unwrap();
    }

    #[test]
    fn test_add_and_list_subscriptions() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");
        insert_channel(&store, "C002");

        store
            .add_subscription("C001", Some("general"), Some(90), true, false)
            .unwrap();
        store
            .add_subscription("C002", Some("random"), None, false, true)
            .unwrap();

        let subs = store.list_subscriptions().unwrap();
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].channel_id, "C001");
        assert_eq!(subs[0].channel_name, Some("general".to_string()));
        assert_eq!(subs[0].retention_days, Some(90));
        assert!(subs[0].sync_threads);
        assert!(!subs[0].sync_members);

        assert_eq!(subs[1].channel_id, "C002");
        assert_eq!(subs[1].channel_name, Some("random".to_string()));
        assert_eq!(subs[1].retention_days, None);
        assert!(!subs[1].sync_threads);
        assert!(subs[1].sync_members);
    }

    #[test]
    fn test_add_subscription_upsert() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .add_subscription("C001", Some("general"), Some(30), true, false)
            .unwrap();
        // Update the same channel
        store
            .add_subscription("C001", Some("general-renamed"), Some(60), false, true)
            .unwrap();

        let subs = store.list_subscriptions().unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].channel_name, Some("general-renamed".to_string()));
        assert_eq!(subs[0].retention_days, Some(60));
        assert!(!subs[0].sync_threads);
        assert!(subs[0].sync_members);
    }

    #[test]
    fn test_remove_subscription() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        store
            .add_subscription("C001", Some("general"), None, true, false)
            .unwrap();
        assert!(store.is_subscribed("C001").unwrap());

        store.remove_subscription("C001").unwrap();
        assert!(!store.is_subscribed("C001").unwrap());

        let subs = store.list_subscriptions().unwrap();
        assert!(subs.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_subscription() {
        let store = Store::open_in_memory().unwrap();
        // Should not error
        store.remove_subscription("C999").unwrap();
    }

    #[test]
    fn test_is_subscribed() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        assert!(!store.is_subscribed("C001").unwrap());

        store
            .add_subscription("C001", Some("general"), None, true, false)
            .unwrap();
        assert!(store.is_subscribed("C001").unwrap());

        assert!(!store.is_subscribed("C999").unwrap());
    }

    #[test]
    fn test_list_subscription_channel_ids() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");
        insert_channel(&store, "C002");
        insert_channel(&store, "C003");

        store
            .add_subscription("C001", Some("general"), None, true, false)
            .unwrap();
        store
            .add_subscription("C002", Some("random"), None, true, false)
            .unwrap();
        store
            .add_subscription("C003", None, None, true, false)
            .unwrap();

        let ids = store.list_subscription_channel_ids().unwrap();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"C001".to_string()));
        assert!(ids.contains(&"C002".to_string()));
        assert!(ids.contains(&"C003".to_string()));
    }

    #[test]
    fn test_sync_state_none_by_default() {
        let store = Store::open_in_memory().unwrap();
        let state = store.get_sync_state("C001").unwrap();
        assert!(state.is_none());
    }

    #[test]
    fn test_update_and_get_sync_state() {
        let store = Store::open_in_memory().unwrap();

        store
            .update_sync_state("C001", Some("1000.0"), Some("2000.0"), None, false)
            .unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert_eq!(state.channel_id, "C001");
        assert_eq!(state.oldest_ts, Some("1000.0".to_string()));
        assert_eq!(state.newest_ts, Some("2000.0".to_string()));
        assert!(!state.is_complete);
        assert!(state.cursor.is_none());
        assert!(state.last_sync > 0);
    }

    #[test]
    fn test_update_sync_state_upsert_preserves_existing() {
        let store = Store::open_in_memory().unwrap();

        // Initial state with oldest_ts
        store
            .update_sync_state("C001", Some("1000.0"), Some("2000.0"), None, false)
            .unwrap();

        // Update with only newest_ts (oldest_ts = None should preserve the existing value)
        store
            .update_sync_state("C001", None, Some("3000.0"), None, false)
            .unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert_eq!(state.oldest_ts, Some("1000.0".to_string())); // preserved
        assert_eq!(state.newest_ts, Some("3000.0".to_string())); // updated
    }

    #[test]
    fn test_update_sync_state_complete() {
        let store = Store::open_in_memory().unwrap();

        store
            .update_sync_state("C001", Some("1000.0"), Some("5000.0"), None, true)
            .unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert!(state.is_complete);
    }

    #[test]
    fn test_sync_state_with_cursor() {
        let store = Store::open_in_memory().unwrap();

        store
            .update_sync_state(
                "C001",
                Some("1000.0"),
                Some("2000.0"),
                Some("dXNlcjpVMDYx"),
                false,
            )
            .unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert_eq!(state.cursor, Some("dXNlcjpVMDYx".to_string()));
        assert!(!state.is_complete);
    }

    #[test]
    fn test_set_sync_cursor_existing() {
        let store = Store::open_in_memory().unwrap();

        // Create initial sync_state
        store
            .update_sync_state("C001", Some("1000.0"), Some("2000.0"), None, false)
            .unwrap();

        // Set cursor for resumable backfill
        store.set_sync_cursor("C001", "page2cursor").unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert_eq!(state.cursor, Some("page2cursor".to_string()));
        // Existing fields should be preserved
        assert_eq!(state.oldest_ts, Some("1000.0".to_string()));
        assert_eq!(state.newest_ts, Some("2000.0".to_string()));
    }

    #[test]
    fn test_set_sync_cursor_no_existing_state() {
        let store = Store::open_in_memory().unwrap();

        // set_sync_cursor on a channel with no sync_state should create one
        store.set_sync_cursor("C001", "initial_cursor").unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert_eq!(state.cursor, Some("initial_cursor".to_string()));
        assert!(!state.is_complete);
        assert!(state.oldest_ts.is_none());
        assert!(state.newest_ts.is_none());
    }

    #[test]
    fn test_set_sync_cursor_updates_last_sync() {
        let store = Store::open_in_memory().unwrap();

        store
            .update_sync_state("C001", Some("1000.0"), None, None, false)
            .unwrap();
        let first_sync = store.get_sync_state("C001").unwrap().unwrap().last_sync;

        // Small delay to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(5));

        store.set_sync_cursor("C001", "new_cursor").unwrap();
        let second_sync = store.get_sync_state("C001").unwrap().unwrap().last_sync;

        assert!(second_sync >= first_sync);
    }

    #[test]
    fn test_delete_old_messages() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        // Insert messages
        {
            let conn = store.conn.lock().unwrap();
            for i in 1..=5 {
                conn.execute(
                    "INSERT INTO messages (channel_id, ts, user_id, text, rendered, synced_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        "C001",
                        format!("{}.000000", i * 1000),
                        "U001",
                        format!("msg {}", i),
                        format!("msg {}", i),
                        epoch_ms(),
                    ],
                )
                .unwrap();
            }
        }

        // Delete messages older than ts 3500 (should delete 1000, 2000, 3000)
        let deleted = store.delete_old_messages("C001", "3500.000000").unwrap();
        assert_eq!(deleted, 3);

        // Verify remaining messages
        let conn = store.conn.lock().unwrap();
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE channel_id = ?1",
                params!["C001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 2);
    }

    #[test]
    fn test_delete_old_messages_none_to_delete() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");

        // Insert a message with a recent ts
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO messages (channel_id, ts, user_id, text, rendered, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params!["C001", "9999.000000", "U001", "recent", "recent", epoch_ms()],
            )
            .unwrap();
        }

        let deleted = store.delete_old_messages("C001", "1000.000000").unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_delete_old_messages_empty_channel() {
        let store = Store::open_in_memory().unwrap();
        let deleted = store.delete_old_messages("C999", "9999.000000").unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_delete_old_messages_only_affects_target_channel() {
        let store = Store::open_in_memory().unwrap();
        insert_channel(&store, "C001");
        insert_channel(&store, "C002");

        {
            let conn = store.conn.lock().unwrap();
            // Old message in C001
            conn.execute(
                "INSERT INTO messages (channel_id, ts, user_id, text, rendered, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params!["C001", "1000.000000", "U001", "old in c001", "old in c001", epoch_ms()],
            )
            .unwrap();
            // Old message in C002
            conn.execute(
                "INSERT INTO messages (channel_id, ts, user_id, text, rendered, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params!["C002", "1000.000000", "U001", "old in c002", "old in c002", epoch_ms()],
            )
            .unwrap();
        }

        // Delete only from C001
        let deleted = store.delete_old_messages("C001", "9999.000000").unwrap();
        assert_eq!(deleted, 1);

        // C002 message should still exist
        let conn = store.conn.lock().unwrap();
        let c002_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE channel_id = ?1",
                params!["C002"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(c002_count, 1);
    }

    #[test]
    fn test_cursor_resumable_backfill_flow() {
        let store = Store::open_in_memory().unwrap();

        // Simulate backfill starting: set initial state
        store
            .update_sync_state("C001", None, Some("5000.0"), Some("page1_cursor"), false)
            .unwrap();

        // After processing page 1, update cursor
        store.set_sync_cursor("C001", "page2_cursor").unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert_eq!(state.cursor, Some("page2_cursor".to_string()));
        assert!(!state.is_complete);

        // After processing page 2, update oldest_ts and clear cursor (backfill complete)
        store
            .update_sync_state("C001", Some("100.0"), Some("5000.0"), None, true)
            .unwrap();

        let state = store.get_sync_state("C001").unwrap().unwrap();
        assert!(state.is_complete);
        assert!(state.cursor.is_none());
        assert_eq!(state.oldest_ts, Some("100.0".to_string()));
        assert_eq!(state.newest_ts, Some("5000.0".to_string()));
    }
}
