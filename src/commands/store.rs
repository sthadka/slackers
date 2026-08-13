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
        StoreCommand::Export(opts) => handle_store_export(opts).await,
        StoreCommand::Import(opts) => handle_store_import(opts).await,
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

    // Open via Store::open to ensure schema migrations run on first use
    let store = crate::store::Store::open(&db_path)?;
    let conn = store.connection();
    let conn = conn.lock().map_err(|e| {
        crate::error::SlackersError::Store(format!("lock poisoned: {}", e))
    })?;

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
    eprint!("Type 'yes' to confirm: ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).map_err(|e| {
        crate::error::SlackersError::Store(format!("failed to read confirmation: {}", e))
    })?;

    if input.trim() != "yes" {
        eprintln!("Aborted.");
        return Ok(());
    }

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

/// Combined subscription + sync state entry for `store sub list` output.
#[derive(Serialize)]
struct SubListEntry {
    channel_id: String,
    channel_name: Option<String>,
    subscribed_at: i64,
    retention_days: Option<u32>,
    sync_threads: bool,
    sync_members: bool,
    sync_state: Option<SubSyncState>,
}

/// Sync state portion of a subscription list entry.
#[derive(Serialize)]
struct SubSyncState {
    oldest_ts: Option<String>,
    newest_ts: Option<String>,
    is_complete: bool,
    last_sync: i64,
}

/// Parse a retention string like "30d", "90d" into a number of days.
fn parse_retention_days(s: &str) -> Result<u32> {
    let s = s.trim();
    let num_str = if s.ends_with('d') || s.ends_with('D') {
        &s[..s.len() - 1]
    } else {
        s
    };
    num_str.parse::<u32>().map_err(|_| {
        crate::error::SlackersError::Store(format!(
            "Invalid retention value '{}'. Expected format like '30d' or '90d'.",
            s
        ))
    })
}

async fn handle_store_sub(cmd: StoreSubCommand) -> Result<()> {
    match cmd {
        StoreSubCommand::Add(opts) => handle_store_sub_add(opts).await,
        StoreSubCommand::Remove(opts) => handle_store_sub_remove(opts).await,
        StoreSubCommand::List => handle_store_sub_list().await,
    }
}

async fn handle_store_sub_add(opts: crate::cli::StoreSubAddOptions) -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.clone().unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;
    let client = crate::slack::SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let retention_days = match &opts.retention {
        Some(r) => Some(parse_retention_days(r)?),
        None => None,
    };
    let sync_threads = !opts.no_threads;
    let sync_members = opts.with_members;

    // Handle --pattern: subscribe to all channels matching a glob
    if let Some(pattern) = &opts.pattern {
        return handle_pattern_subscribe(
            &client,
            &store,
            pattern,
            retention_days,
            sync_threads,
            sync_members,
        )
        .await;
    }

    // Handle --dm: subscribe to a DM channel
    if let Some(dm_user) = &opts.dm {
        return handle_dm_subscribe(
            &client,
            &store,
            dm_user,
            retention_days,
            sync_threads,
            sync_members,
        )
        .await;
    }

    // Normal channel subscription
    let channel_input = &opts.channel;
    let channel_id =
        crate::slack::channels::resolve_channel_id(&client, channel_input).await?;

    // Ensure the channel exists in the channels table (FK constraint)
    ensure_channel_in_store(&client, &store, &channel_id).await?;

    let channel_name = channel_input
        .strip_prefix('#')
        .unwrap_or(channel_input)
        .to_string();

    store.add_subscription(
        &channel_id,
        Some(&channel_name),
        retention_days,
        sync_threads,
        sync_members,
    )?;

    #[derive(Serialize)]
    struct SubAddResult {
        ok: bool,
        channel_id: String,
        channel_name: String,
        retention_days: Option<u32>,
        sync_threads: bool,
        sync_members: bool,
    }

    let result = SubAddResult {
        ok: true,
        channel_id,
        channel_name,
        retention_days,
        sync_threads,
        sync_members,
    };

    println!("{}", crate::output::to_json_output(&result));
    Ok(())
}

/// Subscribe to all channels matching a glob pattern (e.g. "eng-*").
async fn handle_pattern_subscribe(
    client: &crate::slack::SlackClient,
    store: &crate::store::Store,
    pattern: &str,
    retention_days: Option<u32>,
    sync_threads: bool,
    sync_members: bool,
) -> Result<()> {
    let channels = crate::slack::channels::list_conversations(
        client,
        None,
        true,
        None,
        true,
        None,
    )
    .await?;

    let glob = glob::Pattern::new(pattern).map_err(|e| {
        crate::error::SlackersError::Store(format!("Invalid glob pattern '{}': {}", pattern, e))
    })?;

    let mut subscribed: Vec<serde_json::Value> = Vec::new();

    for ch in &channels {
        if let Some(name) = &ch.name {
            if glob.matches(name) {
                // Ensure the channel exists in the channels table (FK constraint)
                store.upsert_channel(ch)?;
                store.add_subscription(
                    &ch.id,
                    Some(name),
                    retention_days,
                    sync_threads,
                    sync_members,
                )?;
                subscribed.push(serde_json::json!({
                    "channel_id": ch.id,
                    "channel_name": name,
                }));
            }
        }
    }

    #[derive(Serialize)]
    struct PatternResult {
        ok: bool,
        pattern: String,
        subscribed_count: usize,
        channels: Vec<serde_json::Value>,
    }

    let result = PatternResult {
        ok: true,
        pattern: pattern.to_string(),
        subscribed_count: subscribed.len(),
        channels: subscribed,
    };

    println!("{}", crate::output::to_json_output(&result));
    Ok(())
}

/// Subscribe to a DM channel by resolving a user handle or ID.
async fn handle_dm_subscribe(
    client: &crate::slack::SlackClient,
    store: &crate::store::Store,
    dm_user: &str,
    retention_days: Option<u32>,
    sync_threads: bool,
    sync_members: bool,
) -> Result<()> {
    // Resolve user to get their ID
    let user = crate::slack::users::get_user(client, dm_user).await?;
    let user_id = &user.id;

    // Open DM conversation to get channel ID
    let conv = client.open_conversation(vec![user_id.clone()]).await?;
    let channel_id = conv.id;

    let display_name = user
        .display_name
        .as_deref()
        .or(user.real_name.as_deref())
        .or(user.name.as_deref())
        .unwrap_or(user_id);

    let channel_name = format!("dm-{}", display_name);

    // Ensure the DM channel exists in the channels table (FK constraint)
    let dm_channel = crate::slack::channels::CompactChannel {
        id: channel_id.clone(),
        name: Some(channel_name.clone()),
        user: Some(user_id.clone()),
        user_name: None,
        is_channel: Some(false),
        is_private: Some(false),
        is_im: Some(true),
        is_mpim: Some(false),
        is_member: Some(true),
        is_archived: Some(false),
        topic: None,
        purpose: None,
        num_members: None,
        created: None,
    };
    store.upsert_channel(&dm_channel)?;

    store.add_subscription(
        &channel_id,
        Some(&channel_name),
        retention_days,
        sync_threads,
        sync_members,
    )?;

    #[derive(Serialize)]
    struct DmSubResult {
        ok: bool,
        channel_id: String,
        channel_name: String,
        user_id: String,
        retention_days: Option<u32>,
        sync_threads: bool,
        sync_members: bool,
    }

    let result = DmSubResult {
        ok: true,
        channel_id,
        channel_name,
        user_id: user_id.clone(),
        retention_days,
        sync_threads,
        sync_members,
    };

    println!("{}", crate::output::to_json_output(&result));
    Ok(())
}

async fn handle_store_sub_remove(opts: crate::cli::StoreSubRemoveOptions) -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.clone().unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;

    let channel_input = &opts.channel;

    // Resolve channel locally — no need to hit the API for a remove operation
    let channel_id = resolve_channel_locally(&store, channel_input)?;

    store.remove_subscription(&channel_id)?;

    #[derive(Serialize)]
    struct SubRemoveResult {
        ok: bool,
        channel_id: String,
        message: String,
    }

    let channel_name = channel_input
        .strip_prefix('#')
        .unwrap_or(channel_input)
        .to_string();

    let result = SubRemoveResult {
        ok: true,
        channel_id,
        message: format!("Unsubscribed from #{}", channel_name),
    };

    println!("{}", crate::output::to_json_output(&result));
    Ok(())
}

async fn handle_store_sub_list() -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;

    let subs = store.list_subscriptions()?;

    let mut entries: Vec<SubListEntry> = Vec::new();
    for sub in &subs {
        let sync_state = store.get_sync_state(&sub.channel_id)?;
        entries.push(SubListEntry {
            channel_id: sub.channel_id.clone(),
            channel_name: sub.channel_name.clone(),
            subscribed_at: sub.subscribed_at,
            retention_days: sub.retention_days,
            sync_threads: sub.sync_threads,
            sync_members: sub.sync_members,
            sync_state: sync_state.map(|s| SubSyncState {
                oldest_ts: s.oldest_ts,
                newest_ts: s.newest_ts,
                is_complete: s.is_complete,
                last_sync: s.last_sync,
            }),
        });
    }

    println!("{}", crate::output::to_json_output(&entries));
    Ok(())
}

async fn handle_store_export(opts: crate::cli::StoreExportOptions) -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.clone().unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;

    // Determine which channels to export
    let channel_ids: Vec<String> = if let Some(ref ch) = opts.channel {
        let client = crate::slack::SlackClient::new(auth_result.auth, auth_result.workspace_url);
        let id = crate::slack::channels::resolve_channel_id(&client, ch).await?;
        vec![id]
    } else {
        store.list_subscription_channel_ids()?
    };

    // Collect all messages
    let mut all_messages: Vec<serde_json::Value> = Vec::new();
    for ch_id in &channel_ids {
        let messages = store.list_messages(ch_id, None, None, u32::MAX)?;
        for msg in messages {
            all_messages.push(serde_json::to_value(&msg).unwrap_or_default());
        }
    }

    let output_str = if opts.format == "csv" {
        // CSV format: ts,channel_id,user_id,text
        let mut lines = vec!["ts,channel_id,user_id,text".to_string()];
        for msg in &all_messages {
            let ts = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");
            let ch = msg.get("channel_id").and_then(|v| v.as_str()).unwrap_or("");
            let user = msg.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
            let text = msg.get("text").and_then(|v| v.as_str()).unwrap_or("")
                .replace('"', "\"\"");
            lines.push(format!("{},{},{},\"{}\"", ts, ch, user, text));
        }
        lines.join("\n")
    } else {
        serde_json::to_string_pretty(&all_messages).unwrap_or_else(|_| "[]".to_string())
    };

    if let Some(ref path) = opts.output {
        std::fs::write(path, &output_str).map_err(|e| {
            crate::error::SlackersError::Store(format!("Failed to write to {}: {}", path, e))
        })?;
        #[derive(Serialize)]
        struct ExportResult {
            ok: bool,
            messages_exported: usize,
            output_file: String,
        }
        let result = ExportResult {
            ok: true,
            messages_exported: all_messages.len(),
            output_file: path.clone(),
        };
        println!("{}", crate::output::to_json_output(&result));
    } else {
        print!("{}", output_str);
    }

    Ok(())
}

/// Resolve a channel input (name or ID) using only the local store.
/// If it looks like a channel ID (starts with C/D/G), returns it directly.
/// Otherwise, looks up the channel by name in the local store or subscriptions.
fn resolve_channel_locally(store: &crate::store::Store, input: &str) -> Result<String> {
    let name = input.strip_prefix('#').unwrap_or(input);

    // If it looks like a channel ID, return it directly
    if name.starts_with('C') || name.starts_with('D') || name.starts_with('G') {
        return Ok(name.to_string());
    }

    // Try the channels table
    if let Some(ch) = store.get_channel_by_name(name)? {
        return Ok(ch.id);
    }

    // Try matching against subscription channel_name
    let subs = store.list_subscriptions()?;
    for sub in &subs {
        if sub.channel_name.as_deref() == Some(name) {
            return Ok(sub.channel_id.clone());
        }
    }

    Err(crate::error::SlackersError::Store(format!(
        "Channel '{}' not found in local store. Use the channel ID (C...) instead.",
        input
    )))
}

/// Fetch channel info from the API and upsert it into the store, ensuring
/// the channels table has a row for the given channel_id (needed for FK constraints).
async fn ensure_channel_in_store(
    client: &crate::slack::SlackClient,
    store: &crate::store::Store,
    channel_id: &str,
) -> Result<()> {
    if store.get_channel_by_id(channel_id)?.is_some() {
        return Ok(());
    }
    let channel_info =
        crate::slack::channels::get_conversation_info(client, channel_id, false).await?;
    store.upsert_channel(&channel_info)?;
    Ok(())
}

async fn handle_store_import(opts: crate::cli::StoreImportOptions) -> Result<()> {
    let auth_result = crate::auth::resolve_auth(None)?;
    let workspace_url = auth_result.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;

    let content = std::fs::read_to_string(&opts.file).map_err(|e| {
        crate::error::SlackersError::Store(format!("Failed to read {}: {}", opts.file, e))
    })?;

    let messages: Vec<serde_json::Value> = serde_json::from_str(&content).map_err(|e| {
        crate::error::SlackersError::Store(format!("Invalid JSON in {}: {}", opts.file, e))
    })?;

    let mut imported = 0usize;
    for msg in &messages {
        let channel_id = msg.get("channel_id").and_then(|v| v.as_str()).unwrap_or("");
        let ts = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");

        if channel_id.is_empty() || ts.is_empty() {
            continue;
        }

        let user_id = msg.get("user_id").and_then(|v| v.as_str());
        let thread_ts = msg.get("thread_ts").and_then(|v| v.as_str());
        let text = msg.get("text").and_then(|v| v.as_str());
        let rendered = msg.get("rendered").and_then(|v| v.as_str());
        let subtype = msg.get("subtype").and_then(|v| v.as_str());
        let reply_count = msg.get("reply_count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let raw_json = msg.get("raw_json").and_then(|v| v.as_str());

        store.upsert_message(
            channel_id, ts, user_id, thread_ts, text, rendered, subtype, reply_count, raw_json,
        )?;
        imported += 1;
    }

    #[derive(Serialize)]
    struct ImportResult {
        ok: bool,
        messages_imported: usize,
        source_file: String,
    }

    let result = ImportResult {
        ok: true,
        messages_imported: imported,
        source_file: opts.file,
    };

    println!("{}", crate::output::to_json_output(&result));
    Ok(())
}
