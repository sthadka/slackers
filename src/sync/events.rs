use serde_json::Value;

/// Parsed Slack WebSocket event.
///
/// Covers all event types referenced in the spec (local-store-sync-plan.md, §A.13).
/// Events that do not match a known type are captured as `Unknown`.
#[derive(Debug, Clone)]
pub enum SlackEvent {
    /// A new message in a channel.
    Message {
        channel: String,
        user: Option<String>,
        text: Option<String>,
        ts: String,
        thread_ts: Option<String>,
        subtype: Option<String>,
        files: Vec<FileInfo>,
        reply_count: Option<i32>,
    },
    /// An existing message was edited.
    MessageChanged {
        channel: String,
        message: EditedMessage,
    },
    /// A message was deleted.
    MessageDeleted {
        channel: String,
        deleted_ts: String,
    },
    /// A reaction was added to a message.
    ReactionAdded {
        user: String,
        reaction: String,
        channel: String,
        ts: String,
    },
    /// A reaction was removed from a message.
    ReactionRemoved {
        user: String,
        reaction: String,
        channel: String,
        ts: String,
    },
    /// A new channel was created.
    ChannelCreated {
        id: String,
        name: String,
        created: u64,
        creator: String,
    },
    /// A channel was renamed.
    ChannelRename {
        id: String,
        name: String,
    },
    /// A channel was archived.
    ChannelArchive {
        channel: String,
    },
    /// A channel was un-archived.
    ChannelUnarchive {
        channel: String,
    },
    /// A user joined a channel.
    MemberJoined {
        user: String,
        channel: String,
    },
    /// A user left a channel.
    MemberLeft {
        user: String,
        channel: String,
    },
    /// A user profile was updated.
    UserChange {
        user: UserInfo,
    },
    /// A message was pinned.
    PinAdded {
        user: String,
        channel_id: String,
        message_ts: Option<String>,
    },
    /// A message was unpinned.
    PinRemoved {
        user: String,
        channel_id: String,
        message_ts: Option<String>,
    },
    /// A file was shared to a channel.
    FileShared {
        file_id: String,
        channel_id: String,
        file: Option<FileInfo>,
    },
    /// A file was deleted.
    FileDeleted {
        file_id: String,
    },
    /// Connection established acknowledgment.
    Hello,
    /// Server is shutting down; reconnect immediately.
    Goodbye,
    /// Hint for the URL to use on next reconnect.
    ReconnectUrl {
        url: String,
    },
    /// Reply to a ping keepalive.
    Pong {
        reply_to: u64,
    },
    /// Any event type not explicitly handled.
    Unknown {
        event_type: String,
    },
}

/// Inner struct for an edited message payload.
#[derive(Debug, Clone)]
pub struct EditedMessage {
    pub user: Option<String>,
    pub text: Option<String>,
    pub ts: String,
    pub edited_user: Option<String>,
    pub edited_ts: Option<String>,
}

/// Minimal user info from a `user_change` event.
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub id: String,
    pub name: Option<String>,
    pub real_name: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub title: Option<String>,
}

/// File metadata attached to a message or file_shared event.
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub id: String,
    pub name: Option<String>,
    pub mimetype: Option<String>,
    pub size: Option<i64>,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
}

// ─── helpers ────────────────────────────────────────────────────────────────

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn require_str(v: &Value, key: &str) -> String {
    str_field(v, key).unwrap_or_default()
}

fn parse_file_info(v: &Value) -> FileInfo {
    FileInfo {
        id: require_str(v, "id"),
        name: str_field(v, "name"),
        mimetype: str_field(v, "mimetype"),
        size: v.get("size").and_then(|x| x.as_i64()),
        url_private: str_field(v, "url_private"),
        url_private_download: str_field(v, "url_private_download"),
    }
}

fn parse_files(v: &Value) -> Vec<FileInfo> {
    v.get("files")
        .and_then(|f| f.as_array())
        .map(|arr| arr.iter().map(parse_file_info).collect())
        .unwrap_or_default()
}

fn parse_user_info(v: &Value) -> UserInfo {
    let profile = v.get("profile");
    UserInfo {
        id: require_str(v, "id"),
        name: str_field(v, "name"),
        real_name: str_field(v, "real_name"),
        display_name: profile
            .and_then(|p| str_field(p, "display_name")),
        email: profile
            .and_then(|p| str_field(p, "email")),
        title: profile
            .and_then(|p| str_field(p, "title")),
    }
}

/// Extract `item.channel` and `item.ts` from a reaction event.
fn reaction_item(v: &Value) -> (String, String) {
    let item = v.get("item").unwrap_or(v);
    (require_str(item, "channel"), require_str(item, "ts"))
}

/// Extract `item.message.ts` from a pin event (may be absent).
fn pin_message_ts(v: &Value) -> Option<String> {
    v.get("item")
        .and_then(|item| item.get("message"))
        .and_then(|msg| str_field(msg, "ts"))
}

// ─── parser ─────────────────────────────────────────────────────────────────

impl SlackEvent {
    /// Parse a raw `serde_json::Value` into a typed `SlackEvent`.
    ///
    /// Falls back to `Unknown` for any unrecognised `type` value.
    pub fn parse(value: &Value) -> SlackEvent {
        let event_type = value
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");

        match event_type {
            "hello" => SlackEvent::Hello,
            "goodbye" => SlackEvent::Goodbye,
            "reconnect_url" => SlackEvent::ReconnectUrl {
                url: require_str(value, "url"),
            },
            "pong" => SlackEvent::Pong {
                reply_to: value
                    .get("reply_to")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
            },
            "message" => Self::parse_message(value),
            "reaction_added" => {
                let (channel, ts) = reaction_item(value);
                SlackEvent::ReactionAdded {
                    user: require_str(value, "user"),
                    reaction: require_str(value, "reaction"),
                    channel,
                    ts,
                }
            }
            "reaction_removed" => {
                let (channel, ts) = reaction_item(value);
                SlackEvent::ReactionRemoved {
                    user: require_str(value, "user"),
                    reaction: require_str(value, "reaction"),
                    channel,
                    ts,
                }
            }
            "channel_created" => {
                let ch = value.get("channel").unwrap_or(value);
                SlackEvent::ChannelCreated {
                    id: require_str(ch, "id"),
                    name: require_str(ch, "name"),
                    created: ch
                        .get("created")
                        .and_then(|c| c.as_u64())
                        .unwrap_or(0),
                    creator: require_str(ch, "creator"),
                }
            }
            "channel_rename" => {
                let ch = value.get("channel").unwrap_or(value);
                SlackEvent::ChannelRename {
                    id: require_str(ch, "id"),
                    name: require_str(ch, "name"),
                }
            }
            "channel_archive" => SlackEvent::ChannelArchive {
                channel: require_str(value, "channel"),
            },
            "channel_unarchive" => SlackEvent::ChannelUnarchive {
                channel: require_str(value, "channel"),
            },
            "member_joined_channel" => SlackEvent::MemberJoined {
                user: require_str(value, "user"),
                channel: require_str(value, "channel"),
            },
            "member_left_channel" => SlackEvent::MemberLeft {
                user: require_str(value, "user"),
                channel: require_str(value, "channel"),
            },
            "user_change" => {
                let user_val = value.get("user").unwrap_or(value);
                SlackEvent::UserChange {
                    user: parse_user_info(user_val),
                }
            }
            "pin_added" => SlackEvent::PinAdded {
                user: require_str(value, "user"),
                channel_id: require_str(value, "channel_id"),
                message_ts: pin_message_ts(value),
            },
            "pin_removed" => SlackEvent::PinRemoved {
                user: require_str(value, "user"),
                channel_id: require_str(value, "channel_id"),
                message_ts: pin_message_ts(value),
            },
            "file_shared" => {
                let file = value.get("file").map(parse_file_info);
                SlackEvent::FileShared {
                    file_id: require_str(value, "file_id"),
                    channel_id: require_str(value, "channel_id"),
                    file,
                }
            }
            "file_deleted" => SlackEvent::FileDeleted {
                file_id: require_str(value, "file_id"),
            },
            _ => SlackEvent::Unknown {
                event_type: event_type.to_string(),
            },
        }
    }

    /// Dispatch on the `subtype` field of a `"type": "message"` payload.
    fn parse_message(value: &Value) -> SlackEvent {
        let subtype = value.get("subtype").and_then(|s| s.as_str());
        match subtype {
            Some("message_changed") => {
                let msg = value.get("message").unwrap_or(value);
                let edited = msg.get("edited");
                SlackEvent::MessageChanged {
                    channel: require_str(value, "channel"),
                    message: EditedMessage {
                        user: str_field(msg, "user"),
                        text: str_field(msg, "text"),
                        ts: require_str(msg, "ts"),
                        edited_user: edited.and_then(|e| str_field(e, "user")),
                        edited_ts: edited.and_then(|e| str_field(e, "ts")),
                    },
                }
            }
            Some("message_deleted") => SlackEvent::MessageDeleted {
                channel: require_str(value, "channel"),
                deleted_ts: require_str(value, "deleted_ts"),
            },
            _ => SlackEvent::Message {
                channel: require_str(value, "channel"),
                user: str_field(value, "user"),
                text: str_field(value, "text"),
                ts: require_str(value, "ts"),
                thread_ts: str_field(value, "thread_ts"),
                subtype: subtype.map(|s| s.to_string()),
                files: parse_files(value),
                reply_count: value.get("reply_count").and_then(|r| r.as_i64()).map(|r| r as i32),
            },
        }
    }

    /// Return the channel ID associated with this event, if any.
    ///
    /// Used by the event loop to filter events for subscribed channels.
    pub fn channel(&self) -> Option<&str> {
        match self {
            SlackEvent::Message { channel, .. } => Some(channel),
            SlackEvent::MessageChanged { channel, .. } => Some(channel),
            SlackEvent::MessageDeleted { channel, .. } => Some(channel),
            SlackEvent::ReactionAdded { channel, .. } => Some(channel),
            SlackEvent::ReactionRemoved { channel, .. } => Some(channel),
            SlackEvent::ChannelArchive { channel } => Some(channel),
            SlackEvent::ChannelUnarchive { channel } => Some(channel),
            SlackEvent::MemberJoined { channel, .. } => Some(channel),
            SlackEvent::MemberLeft { channel, .. } => Some(channel),
            SlackEvent::PinAdded { channel_id, .. } => Some(channel_id),
            SlackEvent::PinRemoved { channel_id, .. } => Some(channel_id),
            SlackEvent::FileShared { channel_id, .. } => Some(channel_id),
            // Channel-level events that don't map to a single subscribed channel:
            SlackEvent::ChannelCreated { .. }
            | SlackEvent::ChannelRename { .. }
            | SlackEvent::UserChange { .. }
            | SlackEvent::FileDeleted { .. }
            | SlackEvent::Hello
            | SlackEvent::Goodbye
            | SlackEvent::ReconnectUrl { .. }
            | SlackEvent::Pong { .. }
            | SlackEvent::Unknown { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_hello() {
        let v = json!({"type": "hello"});
        assert!(matches!(SlackEvent::parse(&v), SlackEvent::Hello));
    }

    #[test]
    fn test_parse_goodbye() {
        let v = json!({"type": "goodbye"});
        assert!(matches!(SlackEvent::parse(&v), SlackEvent::Goodbye));
    }

    #[test]
    fn test_parse_pong() {
        let v = json!({"type": "pong", "reply_to": 42});
        match SlackEvent::parse(&v) {
            SlackEvent::Pong { reply_to } => assert_eq!(reply_to, 42),
            other => panic!("expected Pong, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_reconnect_url() {
        let v = json!({"type": "reconnect_url", "url": "wss://example.com"});
        match SlackEvent::parse(&v) {
            SlackEvent::ReconnectUrl { url } => assert_eq!(url, "wss://example.com"),
            other => panic!("expected ReconnectUrl, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_new_message() {
        let v = json!({
            "type": "message",
            "channel": "C001",
            "user": "U001",
            "text": "hello",
            "ts": "123.456",
            "thread_ts": "123.000"
        });
        match SlackEvent::parse(&v) {
            SlackEvent::Message {
                channel, user, text, ts, thread_ts, subtype, ..
            } => {
                assert_eq!(channel, "C001");
                assert_eq!(user, Some("U001".to_string()));
                assert_eq!(text, Some("hello".to_string()));
                assert_eq!(ts, "123.456");
                assert_eq!(thread_ts, Some("123.000".to_string()));
                assert!(subtype.is_none());
            }
            other => panic!("expected Message, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message_with_files() {
        let v = json!({
            "type": "message",
            "channel": "C001",
            "user": "U001",
            "text": "file here",
            "ts": "100.000",
            "files": [
                { "id": "F001", "name": "doc.pdf", "mimetype": "application/pdf", "size": 1024 }
            ]
        });
        match SlackEvent::parse(&v) {
            SlackEvent::Message { files, .. } => {
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].id, "F001");
                assert_eq!(files[0].name.as_deref(), Some("doc.pdf"));
                assert_eq!(files[0].size, Some(1024));
            }
            other => panic!("expected Message, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message_changed() {
        let v = json!({
            "type": "message",
            "subtype": "message_changed",
            "channel": "C001",
            "message": {
                "user": "U001",
                "text": "edited text",
                "ts": "123.456",
                "edited": { "user": "U001", "ts": "124.000" }
            }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::MessageChanged { channel, message } => {
                assert_eq!(channel, "C001");
                assert_eq!(message.ts, "123.456");
                assert_eq!(message.text, Some("edited text".to_string()));
                assert_eq!(message.edited_user, Some("U001".to_string()));
                assert_eq!(message.edited_ts, Some("124.000".to_string()));
            }
            other => panic!("expected MessageChanged, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message_deleted() {
        let v = json!({
            "type": "message",
            "subtype": "message_deleted",
            "channel": "C001",
            "deleted_ts": "123.456"
        });
        match SlackEvent::parse(&v) {
            SlackEvent::MessageDeleted { channel, deleted_ts } => {
                assert_eq!(channel, "C001");
                assert_eq!(deleted_ts, "123.456");
            }
            other => panic!("expected MessageDeleted, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_reaction_added() {
        let v = json!({
            "type": "reaction_added",
            "user": "U001",
            "reaction": "thumbsup",
            "item": { "type": "message", "channel": "C001", "ts": "123.456" }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::ReactionAdded { user, reaction, channel, ts } => {
                assert_eq!(user, "U001");
                assert_eq!(reaction, "thumbsup");
                assert_eq!(channel, "C001");
                assert_eq!(ts, "123.456");
            }
            other => panic!("expected ReactionAdded, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_reaction_removed() {
        let v = json!({
            "type": "reaction_removed",
            "user": "U001",
            "reaction": "thumbsup",
            "item": { "type": "message", "channel": "C001", "ts": "123.456" }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::ReactionRemoved { user, reaction, channel, ts } => {
                assert_eq!(user, "U001");
                assert_eq!(reaction, "thumbsup");
                assert_eq!(channel, "C001");
                assert_eq!(ts, "123.456");
            }
            other => panic!("expected ReactionRemoved, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_channel_created() {
        let v = json!({
            "type": "channel_created",
            "channel": { "id": "C999", "name": "new-ch", "created": 1700000000, "creator": "U001" }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::ChannelCreated { id, name, created, creator } => {
                assert_eq!(id, "C999");
                assert_eq!(name, "new-ch");
                assert_eq!(created, 1700000000);
                assert_eq!(creator, "U001");
            }
            other => panic!("expected ChannelCreated, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_channel_rename() {
        let v = json!({
            "type": "channel_rename",
            "channel": { "id": "C999", "name": "renamed-ch" }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::ChannelRename { id, name } => {
                assert_eq!(id, "C999");
                assert_eq!(name, "renamed-ch");
            }
            other => panic!("expected ChannelRename, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_member_joined() {
        let v = json!({
            "type": "member_joined_channel",
            "user": "U001",
            "channel": "C001"
        });
        match SlackEvent::parse(&v) {
            SlackEvent::MemberJoined { user, channel } => {
                assert_eq!(user, "U001");
                assert_eq!(channel, "C001");
            }
            other => panic!("expected MemberJoined, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_user_change() {
        let v = json!({
            "type": "user_change",
            "user": {
                "id": "U001",
                "name": "alice",
                "real_name": "Alice Smith",
                "profile": {
                    "display_name": "alice",
                    "email": "alice@example.com",
                    "title": "Engineer"
                }
            }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::UserChange { user } => {
                assert_eq!(user.id, "U001");
                assert_eq!(user.name.as_deref(), Some("alice"));
                assert_eq!(user.real_name.as_deref(), Some("Alice Smith"));
                assert_eq!(user.display_name.as_deref(), Some("alice"));
                assert_eq!(user.email.as_deref(), Some("alice@example.com"));
                assert_eq!(user.title.as_deref(), Some("Engineer"));
            }
            other => panic!("expected UserChange, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_pin_added() {
        let v = json!({
            "type": "pin_added",
            "user": "U001",
            "channel_id": "C001",
            "item": {
                "type": "message",
                "message": { "ts": "123.456" }
            }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::PinAdded { user, channel_id, message_ts } => {
                assert_eq!(user, "U001");
                assert_eq!(channel_id, "C001");
                assert_eq!(message_ts, Some("123.456".to_string()));
            }
            other => panic!("expected PinAdded, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_file_shared() {
        let v = json!({
            "type": "file_shared",
            "file_id": "F001",
            "channel_id": "C001",
            "file": {
                "id": "F001",
                "name": "doc.pdf",
                "mimetype": "application/pdf",
                "size": 5000,
                "url_private": "https://files.slack.com/priv",
                "url_private_download": "https://files.slack.com/dl"
            }
        });
        match SlackEvent::parse(&v) {
            SlackEvent::FileShared { file_id, channel_id, file } => {
                assert_eq!(file_id, "F001");
                assert_eq!(channel_id, "C001");
                let f = file.unwrap();
                assert_eq!(f.id, "F001");
                assert_eq!(f.name.as_deref(), Some("doc.pdf"));
                assert_eq!(f.url_private.as_deref(), Some("https://files.slack.com/priv"));
            }
            other => panic!("expected FileShared, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_file_deleted() {
        let v = json!({"type": "file_deleted", "file_id": "F001"});
        match SlackEvent::parse(&v) {
            SlackEvent::FileDeleted { file_id } => assert_eq!(file_id, "F001"),
            other => panic!("expected FileDeleted, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_unknown() {
        let v = json!({"type": "desktop_notification"});
        match SlackEvent::parse(&v) {
            SlackEvent::Unknown { event_type } => assert_eq!(event_type, "desktop_notification"),
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_channel_accessor() {
        let msg = json!({
            "type": "message", "channel": "C001", "user": "U001",
            "text": "hi", "ts": "1.0"
        });
        let event = SlackEvent::parse(&msg);
        assert_eq!(event.channel(), Some("C001"));

        let hello = json!({"type": "hello"});
        assert_eq!(SlackEvent::parse(&hello).channel(), None);
    }
}
