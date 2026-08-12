use crate::cli::{StoreCommand, StoreSubCommand};
use crate::error::Result;
use serde::Serialize;

/// Output struct for `store info`.
#[derive(Serialize)]
struct StoreInfo {
    db_path: String,
    db_size_bytes: u64,
    messages: i64,
    channels: i64,
    users: i64,
    reactions: i64,
    files: i64,
    subscriptions: i64,
    sync_states: Vec<SyncStateInfo>,
}

/// Per-channel sync state summary.
#[derive(Serialize)]
struct SyncStateInfo {
    channel_id: String,
    oldest_ts: Option<String>,
    newest_ts: Option<String>,
    is_complete: bool,
    last_sync: i64,
}

/// Output struct for `store gc`.
#[derive(Serialize)]
struct GcResult {
    messages_deleted: u64,
    orphaned_reactions_deleted: u64,
    orphaned_files_deleted: u64,
    vacuumed: bool,
}

pub async fn handle_store(cmd: StoreCommand) -> Result<()> {
    match cmd {
        StoreCommand::Info => handle_store_info().await,
        StoreCommand::Gc => handle_store_gc().await,
        StoreCommand::Reset => handle_store_reset().await,
        StoreCommand::Sub { subcommand } => handle_store_sub(subcommand).await,
    }
}

/// Helper: count rows in a table via a raw connection.
fn count_table(conn: &rusqlite::Connection, table: &str) -> Result<i64> {
    // Allowlisted table names to prevent SQL injection
    let allowed = [
        "messages",
        "channels",
        "users",
        "reactions",
        "files",
        "subscriptions",
    ];
    if !allowed.contains(&table) {
        return Err(crate::error::SlackersError::Store(format!(
            "unknown table: {}",
            table
        )));
    }
    let sql = format!("SELECT COUNT(*) FROM {}", table);
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(count)
}

/// Helper: list all sync states via a raw connection.
fn list_sync_states(conn: &rusqlite::Connection) -> Result<Vec<SyncStateInfo>> {
    let mut stmt = conn.prepare(
        "SELECT channel_id, oldest_ts, newest_ts, is_complete, last_sync
         FROM sync_state
         ORDER BY last_sync DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SyncStateInfo {
            channel_id: row.get(0)?,
            oldest_ts: row.get(1)?,
            newest_ts: row.get(2)?,
            is_complete: row.get::<_, i32>(3)? != 0,
            last_sync: row.get(4)?,
        })
    })?;
    let mut states = Vec::new();
    for row in rows {
        states.push(row?);
    }
    Ok(states)
}

async fn handle_store_info() -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;

    let db_size_bytes = std::fs::metadata(&db_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Open a raw connection for read-only queries
    let conn = rusqlite::Connection::open(&db_path)?;

    let messages = count_table(&conn, "messages")?;
    let channels = count_table(&conn, "channels")?;
    let users = count_table(&conn, "users")?;
    let reactions = count_table(&conn, "reactions")?;
    let files = count_table(&conn, "files")?;
    let subscriptions = count_table(&conn, "subscriptions")?;
    let sync_states = list_sync_states(&conn)?;

    let info = StoreInfo {
        db_path: db_path.to_string_lossy().to_string(),
        db_size_bytes,
        messages,
        channels,
        users,
        reactions,
        files,
        subscriptions,
        sync_states,
    };

    println!("{}", crate::output::to_json_output(&info));
    Ok(())
}

async fn handle_store_gc() -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;

    let config = crate::app_config::load_app_config();
    let default_retention = config.store.defaults.retention_days;

    // Delete old messages per subscription retention policy
    let subs = store.list_subscriptions()?;
    let mut total_messages_deleted: u64 = 0;

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for sub in &subs {
        let retention_days = sub.retention_days.or(default_retention);
        if let Some(days) = retention_days {
            let cutoff_secs = now_secs.saturating_sub(days as u64 * 86400);
            let before_ts = format!("{}.000000", cutoff_secs);
            let deleted = store.delete_old_messages(&sub.channel_id, &before_ts)?;
            total_messages_deleted += deleted;
        }
    }

    // Delete orphaned reactions and files, then vacuum using a separate connection
    // since Store's conn field is private.
    let conn = rusqlite::Connection::open(&db_path)?;

    let orphaned_reactions: u64 = conn
        .execute(
            "DELETE FROM reactions WHERE NOT EXISTS (
                SELECT 1 FROM messages
                WHERE messages.channel_id = reactions.channel_id
                  AND messages.ts = reactions.message_ts
            )",
            [],
        )
        .map(|n| n as u64)?;

    let orphaned_files: u64 = conn
        .execute(
            "DELETE FROM files WHERE message_ts IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM messages
                WHERE messages.channel_id = files.channel_id
                  AND messages.ts = files.message_ts
            )",
            [],
        )
        .map(|n| n as u64)?;

    // VACUUM if we deleted a significant amount of data
    let total_deleted = total_messages_deleted + orphaned_reactions + orphaned_files;
    let vacuumed = if total_deleted > 0 {
        conn.execute_batch("VACUUM")?;
        true
    } else {
        false
    };

    let result = GcResult {
        messages_deleted: total_messages_deleted,
        orphaned_reactions_deleted: orphaned_reactions,
        orphaned_files_deleted: orphaned_files,
        vacuumed,
    };

    println!("{}", crate::output::to_json_output(&result));
    Ok(())
}

async fn handle_store_reset() -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;

    eprintln!(
        "WARNING: This will delete ALL data in the store at {}",
        db_path.display()
    );
    eprintln!("Resetting store...");

    // Drop all tables via a raw connection, then re-open through Store to re-migrate
    {
        let conn = rusqlite::Connection::open(&db_path)?;
        conn.execute_batch(
            "DROP TABLE IF EXISTS messages_fts;
             DROP TRIGGER IF EXISTS messages_ai;
             DROP TRIGGER IF EXISTS messages_au;
             DROP TRIGGER IF EXISTS messages_ad;
             DROP TABLE IF EXISTS messages_rowid_map;
             DROP TABLE IF EXISTS pins;
             DROP TABLE IF EXISTS saved_items;
             DROP TABLE IF EXISTS reactions;
             DROP TABLE IF EXISTS files;
             DROP TABLE IF EXISTS channel_members;
             DROP TABLE IF EXISTS sync_state;
             DROP TABLE IF EXISTS subscriptions;
             DROP TABLE IF EXISTS messages;
             DROP TABLE IF EXISTS channels;
             DROP TABLE IF EXISTS users;
             DROP TABLE IF EXISTS workspace;
             PRAGMA user_version = 0;",
        )?;
    }

    // Re-open via Store::open which re-runs migrations, recreating all tables
    let _store = crate::store::Store::open(&db_path)?;

    #[derive(Serialize)]
    struct ResetResult {
        ok: bool,
        message: String,
    }

    let result = ResetResult {
        ok: true,
        message: "Store reset successfully. All tables recreated.".to_string(),
    };

    println!("{}", crate::output::to_json_output(&result));
    Ok(())
}

async fn handle_store_sub(cmd: StoreSubCommand) -> Result<()> {
    match cmd {
        StoreSubCommand::Add(_opts) => {
            eprintln!("Not yet implemented. Use `slackers store sub add` once subscription management is available.");
            Ok(())
        }
        StoreSubCommand::Remove(_opts) => {
            eprintln!("Not yet implemented. Use `slackers store sub remove` once subscription management is available.");
            Ok(())
        }
        StoreSubCommand::List => {
            eprintln!("Not yet implemented. Use `slackers store sub list` once subscription management is available.");
            Ok(())
        }
    }
}
