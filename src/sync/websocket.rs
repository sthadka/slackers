use std::collections::HashSet;
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use crate::store::Store;

use super::events::SlackEvent;

/// Type alias for the full-duplex WebSocket stream.
type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Interval between keepalive pings (seconds).
const PING_INTERVAL_SECS: u64 = 30;

/// Maximum reconnect backoff (seconds).
const MAX_BACKOFF_SECS: u64 = 60;

/// Gap threshold above which we warn about possible missed events (seconds).
const GAP_WARN_SECS: u64 = 300; // 5 minutes

// ─── connect ────────────────────────────────────────────────────────────────

/// Call `rtm.connect`, open the WebSocket, and wait for the `hello` event.
///
/// Returns the split WebSocket stream (sink + stream) together with the
/// initial URL (useful for logging / diagnostics).
pub async fn connect_rtm(
    client: &SlackClient,
) -> Result<(SplitSink<WsStream, WsMessage>, SplitStream<WsStream>, String)> {
    let resp = client.api_call("rtm.connect", vec![]).await?;

    if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err_msg = resp
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown error");
        return Err(SlackersError::Other(format!(
            "rtm.connect failed: {}",
            err_msg
        )));
    }

    let url = resp
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or_else(|| SlackersError::Other("rtm.connect did not return a url".into()))?
        .to_string();

    eprintln!("[sync] rtm.connect OK, connecting to WebSocket...");

    // Build a request with the cookie header — Slack's WSS endpoint
    // requires the xoxd cookie for browser-auth connections.
    let request = url
        .parse::<tokio_tungstenite::tungstenite::http::Uri>()
        .map_err(|e| SlackersError::Other(format!("invalid WSS URL: {}", e)))?;

    let mut req_builder = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(&url)
        .header("Host", request.host().unwrap_or("wss-primary.slack.com"))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        );

    if let Some(cookie) = client.browser_cookie() {
        req_builder = req_builder.header(
            "Cookie",
            format!("d={}", urlencoding::encode(cookie)),
        );
    }

    let request = req_builder
        .body(())
        .map_err(|e| SlackersError::Other(format!("failed to build WS request: {}", e)))?;

    let (ws_stream, _response) = connect_async(request)
        .await
        .map_err(|e| SlackersError::Other(format!("WebSocket connect failed: {}", e)))?;

    let (sink, mut stream) = ws_stream.split();

    // Wait for the hello event.
    let hello_timeout = Duration::from_secs(15);
    let hello = time::timeout(hello_timeout, async {
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        let event = SlackEvent::parse(&value);
                        if matches!(event, SlackEvent::Hello) {
                            return Ok(());
                        }
                    }
                }
                Ok(_) => {} // binary/ping/pong frames — ignore
                Err(e) => {
                    return Err(SlackersError::Other(format!(
                        "WebSocket error while waiting for hello: {}",
                        e
                    )));
                }
            }
        }
        Err(SlackersError::Other(
            "WebSocket closed before receiving hello".into(),
        ))
    })
    .await;

    match hello {
        Ok(Ok(())) => {
            eprintln!("[sync] connected to RTM WebSocket");
            Ok((sink, stream, url))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(SlackersError::Other(
            "timed out waiting for hello from WebSocket".into(),
        )),
    }
}

// ─── event loop ─────────────────────────────────────────────────────────────

/// Read events from a connected WebSocket stream, dispatch them to the store.
///
/// Sends keepalive pings every 30 s. Returns when the stream closes or a
/// `goodbye` event is received.
///
/// Return value:
/// - `Ok(true)`  — received `goodbye`, caller should reconnect immediately.
/// - `Ok(false)` — stream ended normally (disconnect).
/// - `Err(e)`    — fatal error.
pub async fn run_event_loop(
    sink: &mut SplitSink<WsStream, WsMessage>,
    stream: &mut SplitStream<WsStream>,
    store: &Store,
    subscribed_channels: &HashSet<String>,
    reconnect_url: &mut Option<String>,
) -> Result<bool> {
    let mut ping_id: u64 = 0;
    let mut ping_timer = time::interval(Duration::from_secs(PING_INTERVAL_SECS));
    // Consume the first (immediate) tick so pings start after PING_INTERVAL_SECS.
    ping_timer.tick().await;

    loop {
        tokio::select! {
            // ── incoming WebSocket message ──
            msg_opt = stream.next() => {
                match msg_opt {
                    Some(Ok(WsMessage::Text(text))) => {
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(value) => {
                                let event = SlackEvent::parse(&value);

                                // Handle reconnect_url hint.
                                if let SlackEvent::ReconnectUrl { ref url } = event {
                                    *reconnect_url = Some(url.clone());
                                    continue;
                                }

                                // Goodbye — signal immediate reconnect.
                                if matches!(event, SlackEvent::Goodbye) {
                                    eprintln!("[sync] received goodbye — reconnecting");
                                    return Ok(true);
                                }

                                // Pong — just log at debug level.
                                if matches!(event, SlackEvent::Pong { .. }) {
                                    continue;
                                }

                                // Filter: only dispatch events for subscribed channels.
                                if let Some(ch) = event.channel() {
                                    if !subscribed_channels.contains(ch) {
                                        continue;
                                    }
                                }

                                dispatch_event(&event, store);
                            }
                            Err(_) => {
                                // Non-JSON text frame — ignore.
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => {
                        eprintln!("[sync] WebSocket closed");
                        return Ok(false);
                    }
                    Some(Ok(_)) => {
                        // Binary, Ping, Pong at transport level — ignore.
                    }
                    Some(Err(e)) => {
                        eprintln!("[sync] WebSocket error: {}", e);
                        return Err(SlackersError::Other(format!("WebSocket error: {}", e)));
                    }
                }
            }

            // ── periodic ping keepalive ──
            _ = ping_timer.tick() => {
                ping_id += 1;
                let ping = serde_json::json!({"type": "ping", "id": ping_id});
                if let Err(e) = sink.send(WsMessage::Text(ping.to_string().into())).await {
                    eprintln!("[sync] failed to send ping: {}", e);
                    return Err(SlackersError::Other(format!("ping send failed: {}", e)));
                }
            }
        }
    }
}

// ─── dispatch ───────────────────────────────────────────────────────────────

/// Route a parsed event to the appropriate `Store` write method.
fn dispatch_event(event: &SlackEvent, store: &Store) {
    // All store calls return Result; log errors but do not abort the loop.
    let result: Result<()> = match event {
        SlackEvent::Message {
            channel,
            user,
            text,
            ts,
            thread_ts,
            subtype,
            files,
            reply_count,
        } => {
            let r = store.upsert_message(
                channel,
                ts,
                user.as_deref(),
                thread_ts.as_deref(),
                text.as_deref(),
                None, // rendered — raw WS events don't include rendered markdown
                subtype.as_deref(),
                reply_count.unwrap_or(0),
                None, // raw_json
            );
            // Upsert attached files.
            for f in files {
                if let Err(e) = store.upsert_file(
                    &f.id,
                    Some(channel.as_str()),
                    Some(ts.as_str()),
                    f.name.as_deref(),
                    f.mimetype.as_deref(),
                    f.size,
                    f.url_private.as_deref(),
                    f.url_private_download.as_deref(),
                    None,
                ) {
                    eprintln!("[sync] store.upsert_file error: {}", e);
                }
            }
            r
        }
        SlackEvent::MessageChanged { channel, message } => {
            store.mark_edited(
                channel,
                &message.ts,
                message.text.as_deref().unwrap_or(""),
                None, // rendered
            )
        }
        SlackEvent::MessageDeleted {
            channel,
            deleted_ts,
        } => store.soft_delete_message(channel, deleted_ts),
        SlackEvent::ReactionAdded {
            user,
            reaction,
            channel,
            ts,
        } => store.upsert_reaction(channel, ts, reaction, user),
        SlackEvent::ReactionRemoved {
            user,
            reaction,
            channel,
            ts,
        } => store.delete_reaction(channel, ts, reaction, user),
        SlackEvent::ChannelCreated {
            id,
            name,
            created,
            creator: _,
        } => {
            let channel = crate::slack::channels::CompactChannel {
                id: id.clone(),
                name: Some(name.clone()),
                is_channel: Some(true),
                is_private: Some(false),
                is_im: Some(false),
                is_mpim: Some(false),
                is_member: Some(false),
                is_archived: Some(false),
                topic: None,
                purpose: None,
                num_members: None,
                created: Some(*created),
                user: None,
                user_name: None,
            };
            store.upsert_channel(&channel)
        }
        SlackEvent::ChannelRename { id, name } => {
            // Fetch existing, update name. If not found, insert a minimal record.
            let existing = store.get_channel_by_id(id);
            match existing {
                Ok(Some(mut ch)) => {
                    ch.name = Some(name.clone());
                    store.upsert_channel(&ch)
                }
                _ => {
                    let ch = crate::slack::channels::CompactChannel {
                        id: id.clone(),
                        name: Some(name.clone()),
                        is_channel: Some(true),
                        is_private: None,
                        is_im: None,
                        is_mpim: None,
                        is_member: None,
                        is_archived: None,
                        topic: None,
                        purpose: None,
                        num_members: None,
                        created: None,
                        user: None,
                        user_name: None,
                    };
                    store.upsert_channel(&ch)
                }
            }
        }
        SlackEvent::ChannelArchive { channel } => {
            update_channel_archived(store, channel, true)
        }
        SlackEvent::ChannelUnarchive { channel } => {
            update_channel_archived(store, channel, false)
        }
        SlackEvent::MemberJoined { user, channel } => {
            // Insert into channel_members; ignore duplicate.
            insert_channel_member(store, channel, user)
        }
        SlackEvent::MemberLeft { user, channel } => {
            remove_channel_member(store, channel, user)
        }
        SlackEvent::UserChange { user } => {
            let compact = crate::slack::users::CompactSlackUser {
                id: user.id.clone(),
                name: user.name.clone(),
                real_name: user.real_name.clone(),
                display_name: user.display_name.clone(),
                email: user.email.clone(),
                title: user.title.clone(),
                tz: None,
                is_bot: None,
                deleted: None,
            };
            store.upsert_user(&compact)
        }
        SlackEvent::PinAdded {
            user,
            channel_id,
            message_ts,
        } => {
            if let Some(ts) = message_ts {
                upsert_pin(store, channel_id, ts, Some(user))
            } else {
                Ok(())
            }
        }
        SlackEvent::PinRemoved {
            channel_id,
            message_ts,
            ..
        } => {
            if let Some(ts) = message_ts {
                delete_pin(store, channel_id, ts)
            } else {
                Ok(())
            }
        }
        SlackEvent::FileShared {
            file_id,
            channel_id,
            file,
        } => {
            if let Some(f) = file {
                store.upsert_file(
                    &f.id,
                    Some(channel_id.as_str()),
                    None,
                    f.name.as_deref(),
                    f.mimetype.as_deref(),
                    f.size,
                    f.url_private.as_deref(),
                    f.url_private_download.as_deref(),
                    None,
                )
            } else {
                store.upsert_file(file_id, Some(channel_id.as_str()), None, None, None, None, None, None, None)
            }
        }
        SlackEvent::FileDeleted { file_id } => {
            delete_file(store, file_id)
        }
        // Hello, Goodbye, ReconnectUrl, Pong, Unknown — handled upstream.
        _ => Ok(()),
    };

    if let Err(e) = result {
        eprintln!("[sync] dispatch error: {}", e);
    }
}

// ─── store helpers that don't have direct Store methods ─────────────────────

/// Update the `is_archived` flag on an existing channel.
fn update_channel_archived(store: &Store, channel_id: &str, archived: bool) -> Result<()> {
    match store.get_channel_by_id(channel_id) {
        Ok(Some(mut ch)) => {
            ch.is_archived = Some(archived);
            store.upsert_channel(&ch)
        }
        _ => Ok(()), // unknown channel — skip
    }
}

/// Insert a row into `channel_members`.
fn insert_channel_member(store: &Store, channel_id: &str, user_id: &str) -> Result<()> {
    store.insert_channel_member(channel_id, user_id)
}

/// Remove a row from `channel_members`.
fn remove_channel_member(store: &Store, channel_id: &str, user_id: &str) -> Result<()> {
    store.remove_channel_member(channel_id, user_id)
}

/// Insert a pin record.
fn upsert_pin(store: &Store, channel_id: &str, message_ts: &str, user: Option<&str>) -> Result<()> {
    store.upsert_pin(channel_id, message_ts, user, None)
}

/// Delete a pin record.
fn delete_pin(store: &Store, channel_id: &str, message_ts: &str) -> Result<()> {
    store.delete_pin(channel_id, message_ts)
}

/// Delete a file record by ID.
fn delete_file(store: &Store, file_id: &str) -> Result<()> {
    store.delete_file(file_id)
}

// ─── reconnect loop ─────────────────────────────────────────────────────────

/// Run the WebSocket event loop with automatic reconnection.
///
/// On disconnect, applies exponential backoff (1 s, 2 s, 4 s, ... 60 s max).
/// On `goodbye`, reconnects immediately.
/// On reconnect, checks for gaps in sync state per channel and logs warnings.
pub async fn run_with_reconnect(
    client: &SlackClient,
    store: &Store,
    subscribed_channels: &HashSet<String>,
) -> Result<()> {
    let mut backoff_secs: u64 = 1;
    let mut reconnect_url: Option<String> = None;

    loop {
        match connect_rtm(client).await {
            Ok((mut sink, mut stream, _url)) => {
                // Reset backoff on successful connect.
                backoff_secs = 1;

                // Check for gaps after reconnect.
                check_for_gaps(store, subscribed_channels);

                let goodbye = run_event_loop(
                    &mut sink,
                    &mut stream,
                    store,
                    subscribed_channels,
                    &mut reconnect_url,
                )
                .await?;

                if goodbye {
                    // Server asked us to reconnect — no backoff.
                    eprintln!("[sync] reconnecting immediately after goodbye");
                    continue;
                }
            }
            Err(e) => {
                eprintln!("[sync] connection failed: {}", e);
            }
        }

        // Exponential backoff before next attempt.
        eprintln!("[sync] reconnecting in {} s ...", backoff_secs);
        time::sleep(Duration::from_secs(backoff_secs)).await;
        backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
    }
}

// ─── gap detection ──────────────────────────────────────────────────────────

/// After a reconnect, check each subscribed channel's newest local ts
/// against the current time. If the gap exceeds 5 minutes, log a warning.
/// (Actual backfill is triggered by the sync daemon layer, not here.)
fn check_for_gaps(store: &Store, subscribed_channels: &HashSet<String>) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for channel_id in subscribed_channels {
        match store.get_sync_state(channel_id) {
            Ok(Some(state)) => {
                if let Some(ref newest_ts) = state.newest_ts {
                    // Slack ts format: "EPOCH.MICRO"
                    if let Some(epoch) = newest_ts.split('.').next() {
                        if let Ok(ts_secs) = epoch.parse::<u64>() {
                            let gap = now.saturating_sub(ts_secs);
                            if gap > GAP_WARN_SECS {
                                eprintln!(
                                    "[sync] gap of {} s for channel {} (newest_ts={}). Backfill recommended.",
                                    gap, channel_id, newest_ts
                                );
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                eprintln!(
                    "[sync] no sync state for channel {} — full backfill needed",
                    channel_id
                );
            }
            Err(e) => {
                eprintln!(
                    "[sync] failed to read sync state for {}: {}",
                    channel_id, e
                );
            }
        }
    }
}
