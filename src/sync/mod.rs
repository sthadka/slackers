pub mod backfill;
pub mod daemon;
pub mod events;
pub mod poller;
pub mod websocket;

pub use backfill::{backfill_all, backfill_channel, incremental_sync, BackfillStats};
pub use daemon::{is_running, remove_pid_file, write_pid_file};

use std::collections::HashSet;

use serde::Serialize;

use crate::app_config::StoreConfig;
use crate::auth::WorkspaceAuth;
use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use crate::store::Store;

/// The sync connection mode: WebSocket (RTM) or REST polling.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SyncMode {
    WebSocket,
    Polling,
}

/// Per-channel sync status information.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelSyncInfo {
    pub channel_id: String,
    pub channel_name: Option<String>,
    pub newest_ts: Option<String>,
    pub oldest_ts: Option<String>,
    pub is_complete: bool,
}

/// Overall sync daemon status.
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub running: bool,
    pub mode: Option<SyncMode>,
    pub channels: Vec<ChannelSyncInfo>,
}

/// The sync daemon: manages real-time sync via WebSocket or periodic polling.
pub struct SyncDaemon {
    client: SlackClient,
    store: Store,
    config: StoreConfig,
    mode: SyncMode,
}

impl SyncDaemon {
    /// Create a new `SyncDaemon`. Auto-selects WebSocket or polling based on
    /// the auth type.
    ///
    /// - `Browser { xoxc_token, xoxd_cookie }` -> WebSocket (RTM)
    /// - `Standard { token }` -> REST polling
    pub fn new(client: SlackClient, store: Store, config: StoreConfig, auth: &WorkspaceAuth) -> Self {
        let mode = match auth {
            WorkspaceAuth::Browser { .. } => SyncMode::WebSocket,
            WorkspaceAuth::Standard { .. } => SyncMode::Polling,
        };
        Self {
            client,
            store,
            config,
            mode,
        }
    }

    /// Start the sync daemon. Blocks until interrupted or an error occurs.
    ///
    /// Writes the PID file on start and removes it on exit (normal or error).
    pub async fn start(&self) -> Result<()> {
        // Write PID file.
        write_pid_file()?;

        // Build the set of subscribed channel IDs.
        let subscribed_channels: HashSet<String> = self
            .store
            .list_subscription_channel_ids()?
            .into_iter()
            .collect();

        if subscribed_channels.is_empty() {
            remove_pid_file()?;
            return Err(SlackersError::Other(
                "no channel subscriptions found — use `slackers store sub add` first".into(),
            ));
        }

        eprintln!(
            "[sync] starting daemon in {:?} mode ({} channels)",
            self.mode,
            subscribed_channels.len()
        );

        let result = match self.mode {
            SyncMode::WebSocket => {
                websocket::run_with_reconnect(&self.client, &self.store, &subscribed_channels)
                    .await
            }
            SyncMode::Polling => {
                let interval_secs = 60; // default 60s polling interval
                poller::run_poller(
                    &self.client,
                    &self.store,
                    &self.config,
                    interval_secs,
                    &subscribed_channels,
                )
                .await
            }
        };

        // Clean up PID file on exit.
        let _ = remove_pid_file();

        result
    }

    /// Return the current sync status: running state, mode, and per-channel info.
    pub fn status(&self) -> Result<SyncStatus> {
        let running = is_running();
        let subscriptions = self.store.list_subscriptions()?;

        let mut channels = Vec::new();
        for sub in &subscriptions {
            let state = self.store.get_sync_state(&sub.channel_id)?;
            channels.push(ChannelSyncInfo {
                channel_id: sub.channel_id.clone(),
                channel_name: sub.channel_name.clone(),
                newest_ts: state.as_ref().and_then(|s| s.newest_ts.clone()),
                oldest_ts: state.as_ref().and_then(|s| s.oldest_ts.clone()),
                is_complete: state.as_ref().map(|s| s.is_complete).unwrap_or(false),
            });
        }

        Ok(SyncStatus {
            running,
            mode: if running { Some(self.mode.clone()) } else { None },
            channels,
        })
    }
}
