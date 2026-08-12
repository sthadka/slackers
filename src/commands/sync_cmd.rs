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
        SyncCommand::Start(_opts) => {
            eprintln!("Not yet implemented. Real-time sync will be available in a future release.");
            eprintln!("Use `slackers sync backfill` to do a one-time historical sync.");
            Ok(())
        }
        SyncCommand::Stop => {
            eprintln!("Not yet implemented. Sync daemon stop will be available in a future release.");
            Ok(())
        }
        SyncCommand::Status => {
            eprintln!("Not yet implemented. Use `slackers store info` to see current sync state.");
            Ok(())
        }
        SyncCommand::Backfill(opts) => handle_backfill(opts).await,
        SyncCommand::Once => handle_once().await,
    }
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
