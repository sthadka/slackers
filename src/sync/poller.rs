use std::collections::HashSet;
use std::time::Duration;

use crate::app_config::StoreConfig;
use crate::error::Result;
use crate::slack::SlackClient;
use crate::store::Store;

use super::backfill::incremental_sync;

/// Run a periodic poller that fetches new messages via `conversations.history`
/// for each subscribed channel.
///
/// Used when the auth token is `xoxb`/`xoxp` (standard tokens) and WebSocket
/// (RTM) is not available. Calls `incremental_sync()` on each tick.
pub async fn run_poller(
    client: &SlackClient,
    store: &Store,
    config: &StoreConfig,
    interval_secs: u64,
    _subscribed_channels: &HashSet<String>,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    // The first tick fires immediately.
    interval.tick().await;

    loop {
        eprintln!("[sync] poller: running incremental sync...");

        match incremental_sync(client, store, config).await {
            Ok(stats) => {
                let total_new: u64 = stats.iter().map(|s| s.messages_added).sum();
                let total_updated: u64 = stats.iter().map(|s| s.messages_updated).sum();
                if total_new > 0 || total_updated > 0 {
                    eprintln!(
                        "[sync] poller: synced {} new, {} updated messages across {} channels",
                        total_new,
                        total_updated,
                        stats.len()
                    );
                }
            }
            Err(e) => {
                eprintln!("[sync] poller: incremental sync failed: {}", e);
                // Continue polling — transient errors should not stop the daemon.
            }
        }

        // Wait for the next tick.
        interval.tick().await;
    }
}
