use crate::error::Result;
use crate::slack::channels::CompactChannel;
use rusqlite::{params, OptionalExtension, Row};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Store;

const CHANNEL_COLUMNS: &str =
    "id, name, is_channel, is_private, is_im, is_mpim, is_member, is_archived, \
     topic, purpose, num_members, created, user_id";

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn bool_to_int(opt: Option<bool>) -> i32 {
    opt.unwrap_or(false) as i32
}

fn int_to_bool(val: i32) -> Option<bool> {
    Some(val != 0)
}

fn row_to_channel(row: &Row) -> rusqlite::Result<CompactChannel> {
    Ok(CompactChannel {
        id: row.get(0)?,
        name: row.get(1)?,
        is_channel: row.get::<_, i32>(2).map(int_to_bool)?,
        is_private: row.get::<_, i32>(3).map(int_to_bool)?,
        is_im: row.get::<_, i32>(4).map(int_to_bool)?,
        is_mpim: row.get::<_, i32>(5).map(int_to_bool)?,
        is_member: row.get::<_, i32>(6).map(int_to_bool)?,
        is_archived: row.get::<_, i32>(7).map(int_to_bool)?,
        topic: row.get(8)?,
        purpose: row.get(9)?,
        num_members: row.get::<_, Option<i64>>(10)?.map(|n| n as u32),
        created: row.get::<_, Option<i64>>(11)?.map(|c| c as u64),
        user: row.get(12)?,
        user_name: None,
    })
}

impl Store {
    /// Insert or replace a channel record.
    pub fn upsert_channel(&self, channel: &CompactChannel) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let now = now_epoch();
        conn.execute(
            &format!(
                "INSERT OR REPLACE INTO channels ({}, synced_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                CHANNEL_COLUMNS
            ),
            params![
                channel.id,
                channel.name,
                bool_to_int(channel.is_channel),
                bool_to_int(channel.is_private),
                bool_to_int(channel.is_im),
                bool_to_int(channel.is_mpim),
                bool_to_int(channel.is_member),
                bool_to_int(channel.is_archived),
                channel.topic,
                channel.purpose,
                channel.num_members.map(|n| n as i64),
                channel.created.map(|c| c as i64),
                channel.user,
                now,
            ],
        )?;
        Ok(())
    }

    /// Get a channel by its Slack ID.
    pub fn get_channel_by_id(&self, id: &str) -> Result<Option<CompactChannel>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM channels WHERE id = ?1",
            CHANNEL_COLUMNS
        ))?;
        let result = stmt.query_row(params![id], row_to_channel).optional()?;
        Ok(result)
    }

    /// Get a channel by its name (exact match).
    pub fn get_channel_by_name(&self, name: &str) -> Result<Option<CompactChannel>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM channels WHERE name = ?1",
            CHANNEL_COLUMNS
        ))?;
        let result = stmt.query_row(params![name], row_to_channel).optional()?;
        Ok(result)
    }

    /// Insert a user into channel_members. Ignores duplicates.
    pub fn insert_channel_member(&self, channel_id: &str, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "INSERT OR IGNORE INTO channel_members (channel_id, user_id) VALUES (?1, ?2)",
            params![channel_id, user_id],
        )?;
        Ok(())
    }

    /// Remove a user from channel_members.
    pub fn remove_channel_member(&self, channel_id: &str, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "DELETE FROM channel_members WHERE channel_id = ?1 AND user_id = ?2",
            params![channel_id, user_id],
        )?;
        Ok(())
    }

    /// Insert or replace a pin record.
    pub fn upsert_pin(
        &self,
        channel_id: &str,
        message_ts: &str,
        pinned_by: Option<&str>,
        pinned_at: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "INSERT OR REPLACE INTO pins (channel_id, message_ts, pinned_by, pinned_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![channel_id, message_ts, pinned_by, pinned_at],
        )?;
        Ok(())
    }

    /// Delete a pin record.
    pub fn delete_pin(&self, channel_id: &str, message_ts: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        conn.execute(
            "DELETE FROM pins WHERE channel_id = ?1 AND message_ts = ?2",
            params![channel_id, message_ts],
        )?;
        Ok(())
    }

    /// List all channels, ordered by name.
    pub fn list_channels(&self) -> Result<Vec<CompactChannel>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM channels ORDER BY name",
            CHANNEL_COLUMNS
        ))?;
        let rows = stmt.query_map([], row_to_channel)?;
        let mut channels = Vec::new();
        for row in rows {
            channels.push(row?);
        }
        Ok(channels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel(id: &str, name: Option<&str>) -> CompactChannel {
        CompactChannel {
            id: id.to_string(),
            name: name.map(|s| s.to_string()),
            user: None,
            user_name: None,
            is_channel: Some(true),
            is_private: Some(false),
            is_im: None,
            is_mpim: None,
            is_member: Some(true),
            is_archived: Some(false),
            topic: None,
            purpose: None,
            num_members: None,
            created: None,
        }
    }

    #[test]
    fn test_upsert_and_get_channel_by_id() {
        let store = Store::open_in_memory().unwrap();
        let ch = make_channel("C001", Some("general"));
        store.upsert_channel(&ch).unwrap();

        let result = store.get_channel_by_id("C001").unwrap();
        assert!(result.is_some());
        let got = result.unwrap();
        assert_eq!(got.id, "C001");
        assert_eq!(got.name, Some("general".to_string()));
        assert_eq!(got.is_channel, Some(true));
        assert_eq!(got.is_private, Some(false));
        assert_eq!(got.is_member, Some(true));
        assert_eq!(got.is_archived, Some(false));
    }

    #[test]
    fn test_get_channel_by_name() {
        let store = Store::open_in_memory().unwrap();
        let ch = make_channel("C002", Some("random"));
        store.upsert_channel(&ch).unwrap();

        let result = store.get_channel_by_name("random").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "C002");
    }

    #[test]
    fn test_get_channel_by_id_not_found() {
        let store = Store::open_in_memory().unwrap();
        let result = store.get_channel_by_id("C999").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_channel_by_name_not_found() {
        let store = Store::open_in_memory().unwrap();
        let result = store.get_channel_by_name("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_channels() {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_channel(&make_channel("C001", Some("general")))
            .unwrap();
        store
            .upsert_channel(&make_channel("C002", Some("random")))
            .unwrap();
        store
            .upsert_channel(&make_channel("C003", Some("engineering")))
            .unwrap();

        let channels = store.list_channels().unwrap();
        assert_eq!(channels.len(), 3);
        // Ordered by name
        assert_eq!(channels[0].name, Some("engineering".to_string()));
        assert_eq!(channels[1].name, Some("general".to_string()));
        assert_eq!(channels[2].name, Some("random".to_string()));
    }

    #[test]
    fn test_list_channels_empty() {
        let store = Store::open_in_memory().unwrap();
        let channels = store.list_channels().unwrap();
        assert!(channels.is_empty());
    }

    #[test]
    fn test_upsert_channel_updates_existing() {
        let store = Store::open_in_memory().unwrap();
        let mut ch = make_channel("C001", Some("general"));
        ch.topic = Some("Old topic".to_string());
        store.upsert_channel(&ch).unwrap();

        // Update with new topic
        ch.topic = Some("New topic".to_string());
        ch.is_archived = Some(true);
        store.upsert_channel(&ch).unwrap();

        let got = store.get_channel_by_id("C001").unwrap().unwrap();
        assert_eq!(got.topic, Some("New topic".to_string()));
        assert_eq!(got.is_archived, Some(true));
    }

    #[test]
    fn test_channel_with_all_fields() {
        let store = Store::open_in_memory().unwrap();
        let ch = CompactChannel {
            id: "C100".to_string(),
            name: Some("project-x".to_string()),
            user: Some("U001".to_string()),
            user_name: Some("alice".to_string()), // not persisted
            is_channel: Some(true),
            is_private: Some(true),
            is_im: Some(false),
            is_mpim: Some(false),
            is_member: Some(true),
            is_archived: Some(false),
            topic: Some("Project X discussion".to_string()),
            purpose: Some("Coordinate on project X".to_string()),
            num_members: Some(42),
            created: Some(1609459200),
        };
        store.upsert_channel(&ch).unwrap();

        let got = store.get_channel_by_id("C100").unwrap().unwrap();
        assert_eq!(got.id, "C100");
        assert_eq!(got.name, Some("project-x".to_string()));
        assert_eq!(got.user, Some("U001".to_string()));
        assert_eq!(got.user_name, None); // not persisted
        assert_eq!(got.is_channel, Some(true));
        assert_eq!(got.is_private, Some(true));
        assert_eq!(got.is_im, Some(false));
        assert_eq!(got.is_mpim, Some(false));
        assert_eq!(got.is_member, Some(true));
        assert_eq!(got.is_archived, Some(false));
        assert_eq!(got.topic, Some("Project X discussion".to_string()));
        assert_eq!(got.purpose, Some("Coordinate on project X".to_string()));
        assert_eq!(got.num_members, Some(42));
        assert_eq!(got.created, Some(1609459200));
    }

    #[test]
    fn test_insert_channel_member() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_channel(&make_channel("C001", Some("general"))).unwrap();

        store.insert_channel_member("C001", "U001").unwrap();
        store.insert_channel_member("C001", "U002").unwrap();

        // Inserting duplicate should succeed (INSERT OR IGNORE).
        store.insert_channel_member("C001", "U001").unwrap();

        // Verify via raw SQL.
        let conn = store.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_members WHERE channel_id = ?1",
                params!["C001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_remove_channel_member() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_channel(&make_channel("C001", Some("general"))).unwrap();

        store.insert_channel_member("C001", "U001").unwrap();
        store.insert_channel_member("C001", "U002").unwrap();

        store.remove_channel_member("C001", "U001").unwrap();

        let conn = store.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_members WHERE channel_id = ?1",
                params!["C001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_remove_channel_member_nonexistent() {
        let store = Store::open_in_memory().unwrap();
        // Removing a non-existent member should succeed (no-op).
        store.remove_channel_member("C999", "U999").unwrap();
    }

    #[test]
    fn test_upsert_and_delete_pin() {
        let store = Store::open_in_memory().unwrap();

        store.upsert_pin("C001", "1700000000.000001", Some("U001"), Some(1700000000)).unwrap();

        // Verify pin exists.
        let conn = store.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pins WHERE channel_id = ?1 AND message_ts = ?2",
                params!["C001", "1700000000.000001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        drop(conn);

        // Delete pin.
        store.delete_pin("C001", "1700000000.000001").unwrap();

        let conn = store.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pins WHERE channel_id = ?1 AND message_ts = ?2",
                params!["C001", "1700000000.000001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_upsert_pin_replaces() {
        let store = Store::open_in_memory().unwrap();

        store.upsert_pin("C001", "1700000000.000001", Some("U001"), Some(100)).unwrap();
        store.upsert_pin("C001", "1700000000.000001", Some("U002"), Some(200)).unwrap();

        let conn = store.conn.lock().unwrap();
        let pinned_by: String = conn
            .query_row(
                "SELECT pinned_by FROM pins WHERE channel_id = ?1 AND message_ts = ?2",
                params!["C001", "1700000000.000001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pinned_by, "U002");
    }

    #[test]
    fn test_delete_pin_nonexistent() {
        let store = Store::open_in_memory().unwrap();
        // Deleting a non-existent pin should succeed (no-op).
        store.delete_pin("C999", "0.0").unwrap();
    }

    #[test]
    fn test_channel_with_none_booleans() {
        let store = Store::open_in_memory().unwrap();
        let ch = CompactChannel {
            id: "D001".to_string(),
            name: None,
            user: None,
            user_name: None,
            is_channel: None,
            is_private: None,
            is_im: None,
            is_mpim: None,
            is_member: None,
            is_archived: None,
            topic: None,
            purpose: None,
            num_members: None,
            created: None,
        };
        store.upsert_channel(&ch).unwrap();

        let got = store.get_channel_by_id("D001").unwrap().unwrap();
        assert_eq!(got.id, "D001");
        assert_eq!(got.name, None);
        // None bools are stored as 0 (false), read back as Some(false)
        assert_eq!(got.is_channel, Some(false));
        assert_eq!(got.is_private, Some(false));
        assert_eq!(got.num_members, None);
        assert_eq!(got.created, None);
    }
}
