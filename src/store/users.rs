use crate::error::Result;
use crate::slack::users::CompactSlackUser;
use rusqlite::{params, OptionalExtension, Row};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Store;

const USER_COLUMNS: &str =
    "id, name, real_name, display_name, email, title, tz, is_bot, deleted";

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

fn row_to_user(row: &Row) -> rusqlite::Result<CompactSlackUser> {
    let name: Option<String> = row.get(1)?;
    Ok(CompactSlackUser {
        id: row.get(0)?,
        name,
        real_name: row.get(2)?,
        display_name: row.get(3)?,
        email: row.get(4)?,
        title: row.get(5)?,
        tz: row.get(6)?,
        is_bot: row.get::<_, i32>(7).map(int_to_bool)?,
        deleted: row.get::<_, i32>(8).map(int_to_bool)?,
    })
}

impl Store {
    /// Insert or replace a single user record.
    pub fn upsert_user(&self, user: &CompactSlackUser) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let now = now_epoch();
        conn.execute(
            &format!(
                "INSERT OR REPLACE INTO users ({}, synced_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                USER_COLUMNS
            ),
            params![
                user.id,
                user.name.as_deref().unwrap_or(""),
                user.real_name,
                user.display_name,
                user.email,
                user.title,
                user.tz,
                bool_to_int(user.is_bot),
                bool_to_int(user.deleted),
                now,
            ],
        )?;
        Ok(())
    }

    /// Batch insert or replace users in a single transaction.
    pub fn upsert_users(&self, users: &[CompactSlackUser]) -> Result<()> {
        if users.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let now = now_epoch();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(&format!(
                "INSERT OR REPLACE INTO users ({}, synced_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                USER_COLUMNS
            ))?;
            for user in users {
                stmt.execute(params![
                    user.id,
                    user.name.as_deref().unwrap_or(""),
                    user.real_name,
                    user.display_name,
                    user.email,
                    user.title,
                    user.tz,
                    bool_to_int(user.is_bot),
                    bool_to_int(user.deleted),
                    now,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Get a user by their Slack ID.
    pub fn get_user_by_id(&self, id: &str) -> Result<Option<CompactSlackUser>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM users WHERE id = ?1",
            USER_COLUMNS
        ))?;
        let result = stmt.query_row(params![id], row_to_user).optional()?;
        Ok(result)
    }

    /// Get multiple users by their Slack IDs.
    pub fn get_users_by_ids(&self, ids: &[String]) -> Result<Vec<CompactSlackUser>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let placeholders: String = std::iter::repeat("?")
            .take(ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT {} FROM users WHERE id IN ({})",
            USER_COLUMNS, placeholders
        );
        let mut stmt = conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::types::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
        let rows = stmt.query_map(refs.as_slice(), row_to_user)?;
        let mut users = Vec::new();
        for row in rows {
            users.push(row?);
        }
        Ok(users)
    }

    /// Get a user by their handle/name (exact match).
    pub fn get_user_by_name(&self, name: &str) -> Result<Option<CompactSlackUser>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM users WHERE name = ?1",
            USER_COLUMNS
        ))?;
        let result = stmt.query_row(params![name], row_to_user).optional()?;
        Ok(result)
    }

    /// List all users, ordered by name.
    pub fn list_users(&self) -> Result<Vec<CompactSlackUser>> {
        let conn = self.conn.lock().map_err(|e| {
            crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
        })?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM users ORDER BY name",
            USER_COLUMNS
        ))?;
        let rows = stmt.query_map([], row_to_user)?;
        let mut users = Vec::new();
        for row in rows {
            users.push(row?);
        }
        Ok(users)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_user(id: &str, name: &str) -> CompactSlackUser {
        CompactSlackUser {
            id: id.to_string(),
            name: Some(name.to_string()),
            real_name: None,
            display_name: None,
            email: None,
            title: None,
            tz: None,
            is_bot: Some(false),
            deleted: Some(false),
        }
    }

    #[test]
    fn test_upsert_and_get_user_by_id() {
        let store = Store::open_in_memory().unwrap();
        let user = make_user("U001", "alice");
        store.upsert_user(&user).unwrap();

        let result = store.get_user_by_id("U001").unwrap();
        assert!(result.is_some());
        let got = result.unwrap();
        assert_eq!(got.id, "U001");
        assert_eq!(got.name, Some("alice".to_string()));
        assert_eq!(got.is_bot, Some(false));
        assert_eq!(got.deleted, Some(false));
    }

    #[test]
    fn test_get_user_by_name() {
        let store = Store::open_in_memory().unwrap();
        let user = make_user("U002", "bob");
        store.upsert_user(&user).unwrap();

        let result = store.get_user_by_name("bob").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "U002");
    }

    #[test]
    fn test_get_user_by_id_not_found() {
        let store = Store::open_in_memory().unwrap();
        let result = store.get_user_by_id("U999").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_user_by_name_not_found() {
        let store = Store::open_in_memory().unwrap();
        let result = store.get_user_by_name("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_users() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_user(&make_user("U001", "alice")).unwrap();
        store.upsert_user(&make_user("U002", "charlie")).unwrap();
        store.upsert_user(&make_user("U003", "bob")).unwrap();

        let users = store.list_users().unwrap();
        assert_eq!(users.len(), 3);
        // Ordered by name
        assert_eq!(users[0].name, Some("alice".to_string()));
        assert_eq!(users[1].name, Some("bob".to_string()));
        assert_eq!(users[2].name, Some("charlie".to_string()));
    }

    #[test]
    fn test_list_users_empty() {
        let store = Store::open_in_memory().unwrap();
        let users = store.list_users().unwrap();
        assert!(users.is_empty());
    }

    #[test]
    fn test_upsert_user_updates_existing() {
        let store = Store::open_in_memory().unwrap();
        let mut user = make_user("U001", "alice");
        user.real_name = Some("Alice Smith".to_string());
        store.upsert_user(&user).unwrap();

        // Update with new fields
        user.real_name = Some("Alice Johnson".to_string());
        user.title = Some("Engineer".to_string());
        store.upsert_user(&user).unwrap();

        let got = store.get_user_by_id("U001").unwrap().unwrap();
        assert_eq!(got.real_name, Some("Alice Johnson".to_string()));
        assert_eq!(got.title, Some("Engineer".to_string()));
    }

    #[test]
    fn test_upsert_users_batch() {
        let store = Store::open_in_memory().unwrap();
        let users = vec![
            make_user("U001", "alice"),
            make_user("U002", "bob"),
            make_user("U003", "charlie"),
        ];
        store.upsert_users(&users).unwrap();

        let all = store.list_users().unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_upsert_users_batch_empty() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_users(&[]).unwrap();
        let all = store.list_users().unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn test_upsert_users_batch_updates_existing() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_user(&make_user("U001", "alice")).unwrap();

        let users = vec![
            {
                let mut u = make_user("U001", "alice");
                u.title = Some("Manager".to_string());
                u
            },
            make_user("U002", "bob"),
        ];
        store.upsert_users(&users).unwrap();

        let all = store.list_users().unwrap();
        assert_eq!(all.len(), 2);
        let alice = store.get_user_by_id("U001").unwrap().unwrap();
        assert_eq!(alice.title, Some("Manager".to_string()));
    }

    #[test]
    fn test_get_users_by_ids() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_user(&make_user("U001", "alice")).unwrap();
        store.upsert_user(&make_user("U002", "bob")).unwrap();
        store.upsert_user(&make_user("U003", "charlie")).unwrap();

        let ids = vec!["U001".to_string(), "U003".to_string()];
        let users = store.get_users_by_ids(&ids).unwrap();
        assert_eq!(users.len(), 2);

        let user_ids: Vec<&str> = users.iter().map(|u| u.id.as_str()).collect();
        assert!(user_ids.contains(&"U001"));
        assert!(user_ids.contains(&"U003"));
    }

    #[test]
    fn test_get_users_by_ids_empty() {
        let store = Store::open_in_memory().unwrap();
        let users = store.get_users_by_ids(&[]).unwrap();
        assert!(users.is_empty());
    }

    #[test]
    fn test_get_users_by_ids_partial_match() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_user(&make_user("U001", "alice")).unwrap();

        let ids = vec!["U001".to_string(), "U999".to_string()];
        let users = store.get_users_by_ids(&ids).unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].id, "U001");
    }

    #[test]
    fn test_user_with_all_fields() {
        let store = Store::open_in_memory().unwrap();
        let user = CompactSlackUser {
            id: "U100".to_string(),
            name: Some("johnd".to_string()),
            real_name: Some("John Doe".to_string()),
            display_name: Some("JD".to_string()),
            email: Some("john@example.com".to_string()),
            title: Some("Senior Engineer".to_string()),
            tz: Some("America/New_York".to_string()),
            is_bot: Some(false),
            deleted: Some(false),
        };
        store.upsert_user(&user).unwrap();

        let got = store.get_user_by_id("U100").unwrap().unwrap();
        assert_eq!(got.id, "U100");
        assert_eq!(got.name, Some("johnd".to_string()));
        assert_eq!(got.real_name, Some("John Doe".to_string()));
        assert_eq!(got.display_name, Some("JD".to_string()));
        assert_eq!(got.email, Some("john@example.com".to_string()));
        assert_eq!(got.title, Some("Senior Engineer".to_string()));
        assert_eq!(got.tz, Some("America/New_York".to_string()));
        assert_eq!(got.is_bot, Some(false));
        assert_eq!(got.deleted, Some(false));
    }

    #[test]
    fn test_user_with_none_name() {
        let store = Store::open_in_memory().unwrap();
        let user = CompactSlackUser {
            id: "U200".to_string(),
            name: None,
            real_name: None,
            display_name: None,
            email: None,
            title: None,
            tz: None,
            is_bot: None,
            deleted: None,
        };
        store.upsert_user(&user).unwrap();

        let got = store.get_user_by_id("U200").unwrap().unwrap();
        assert_eq!(got.id, "U200");
        // name is stored as "" since DB column is NOT NULL; read back as Some("")
        assert_eq!(got.name, Some("".to_string()));
        assert_eq!(got.real_name, None);
        // None bools are stored as 0, read back as Some(false)
        assert_eq!(got.is_bot, Some(false));
        assert_eq!(got.deleted, Some(false));
    }

    #[test]
    fn test_bot_user() {
        let store = Store::open_in_memory().unwrap();
        let user = CompactSlackUser {
            id: "U300".to_string(),
            name: Some("slackbot".to_string()),
            real_name: Some("Slackbot".to_string()),
            display_name: None,
            email: None,
            title: None,
            tz: None,
            is_bot: Some(true),
            deleted: Some(false),
        };
        store.upsert_user(&user).unwrap();

        let got = store.get_user_by_id("U300").unwrap().unwrap();
        assert_eq!(got.is_bot, Some(true));
        assert_eq!(got.deleted, Some(false));
    }
}
