use crate::cli::SyncCommand;
use crate::error::Result;
use serde::Serialize;

/// Summary output for sync backfill / once commands.
#[derive(Serialize)]
struct SyncSummary {
    channels_synced: usize,
    total_messages_added: u64,
    total_messages_updated: u64,
    channels: Vec<crate::sync::BackfillStats>,
}

pub async fn handle_sync(cmd: SyncCommand, _read_only: bool) -> Result<()> {
    match cmd {
        SyncCommand::Start(opts) => handle_start(opts).await,
        SyncCommand::Stop => handle_stop(),
        SyncCommand::Status => handle_status(),
        SyncCommand::Backfill(opts) => handle_backfill(opts).await,
        SyncCommand::Once => handle_once().await,
    }
}

async fn handle_start(opts: crate::cli::SyncStartOptions) -> Result<()> {
    // Check if already running.
    if crate::sync::daemon::is_running() {
        eprintln!("[sync] daemon is already running");
        return Err(crate::error::SlackersError::Other(
            "sync daemon is already running — use `slackers sync stop` first".into(),
        ));
    }

    let resolved = crate::auth::resolve_auth(None)?;
    let workspace_url = resolved.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;
    let config = crate::app_config::load_app_config();
    let client =
        crate::slack::SlackClient::new(resolved.auth.clone(), Some(workspace_url));

    let daemon =
        crate::sync::SyncDaemon::new(client, store, config.store, &resolved.auth);

    if opts.daemon {
        eprintln!(
            "[sync] --daemon flag noted. True daemonization is not yet supported; \
             the process runs in the foreground. Use `nohup slackers sync start &` \
             to run in the background."
        );
    }

    eprintln!("[sync] starting sync daemon (press Ctrl-C to stop)...");
    daemon.start().await
}

fn handle_stop() -> Result<()> {
    if !crate::sync::daemon::is_running() {
        eprintln!("[sync] daemon is not running");
        return Ok(());
    }

    crate::sync::daemon::stop_daemon()?;
    eprintln!("[sync] daemon stopped");
    Ok(())
}

fn handle_status() -> Result<()> {
    let running = crate::sync::daemon::is_running();

    let resolved = crate::auth::resolve_auth(None)?;
    let workspace_url = resolved.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;
    let config = crate::app_config::load_app_config();
    let client =
        crate::slack::SlackClient::new(resolved.auth.clone(), Some(workspace_url));

    let daemon =
        crate::sync::SyncDaemon::new(client, store, config.store, &resolved.auth);

    let status = daemon.status()?;

    // Override running field with our fresh is_running() check,
    // since the daemon.status() also calls is_running() internally
    // but we want to be consistent.
    let _ = running;

    println!("{}", crate::output::to_json_output(&status));
    Ok(())
}

async fn handle_backfill(opts: crate::cli::SyncBackfillOptions) -> Result<()> {
    let resolved = crate::auth::resolve_auth(None)?;
    let workspace_url = resolved.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;
    let config = crate::app_config::load_app_config();
    let client = crate::slack::SlackClient::new(resolved.auth.clone(), Some(workspace_url));

    let stats = if let Some(ref channel_input) = opts.channel {
        // Backfill a single channel: resolve name to ID first.
        let channel_id =
            crate::slack::resolve_channel_id(&client, channel_input).await?;
        let channel_stats =
            crate::sync::backfill_channel(&client, &store, &channel_id, &config.store).await?;
        vec![channel_stats]
    } else {
        // Backfill all subscribed channels.
        crate::sync::backfill_all(&client, &store, &config.store).await?
    };

    let summary = SyncSummary {
        channels_synced: stats.len(),
        total_messages_added: stats.iter().map(|s| s.messages_added).sum(),
        total_messages_updated: stats.iter().map(|s| s.messages_updated).sum(),
        channels: stats,
    };

    println!("{}", crate::output::to_json_output(&summary));
    Ok(())
}

async fn handle_once() -> Result<()> {
    let resolved = crate::auth::resolve_auth(None)?;
    let workspace_url = resolved.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;
    let config = crate::app_config::load_app_config();
    let client = crate::slack::SlackClient::new(resolved.auth.clone(), Some(workspace_url));

    let stats = crate::sync::incremental_sync(&client, &store, &config.store).await?;

    let summary = SyncSummary {
        channels_synced: stats.len(),
        total_messages_added: stats.iter().map(|s| s.messages_added).sum(),
        total_messages_updated: stats.iter().map(|s| s.messages_updated).sum(),
        channels: stats,
    };

    println!("{}", crate::output::to_json_output(&summary));
    Ok(())
}
