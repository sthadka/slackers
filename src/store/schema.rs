/// Schema version 1: full DDL for all store tables, indexes, FTS5, and triggers.
pub const SCHEMA_V1: &str = r#"
-- Workspace metadata
CREATE TABLE workspace (
    team_id        TEXT PRIMARY KEY,
    team_domain    TEXT NOT NULL,
    workspace_url  TEXT NOT NULL,
    workspace_name TEXT,
    icon_url       TEXT,
    synced_at      INTEGER NOT NULL
);

-- Channels, DMs, MPIMs
CREATE TABLE channels (
    id           TEXT PRIMARY KEY,
    name         TEXT,
    is_channel   INTEGER NOT NULL DEFAULT 0,
    is_private   INTEGER NOT NULL DEFAULT 0,
    is_im        INTEGER NOT NULL DEFAULT 0,
    is_mpim      INTEGER NOT NULL DEFAULT 0,
    is_member    INTEGER NOT NULL DEFAULT 0,
    is_archived  INTEGER NOT NULL DEFAULT 0,
    topic        TEXT,
    purpose      TEXT,
    num_members  INTEGER,
    created      INTEGER,
    creator_id   TEXT,
    user_id      TEXT,
    synced_at    INTEGER NOT NULL,
    UNIQUE(id)
);
CREATE INDEX idx_channels_name ON channels(name);

-- Users
CREATE TABLE users (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    real_name    TEXT,
    display_name TEXT,
    email        TEXT,
    title        TEXT,
    tz           TEXT,
    is_bot       INTEGER NOT NULL DEFAULT 0,
    deleted      INTEGER NOT NULL DEFAULT 0,
    avatar_url   TEXT,
    synced_at    INTEGER NOT NULL
);
CREATE INDEX idx_users_name ON users(name);

-- Messages
CREATE TABLE messages (
    channel_id   TEXT NOT NULL,
    ts           TEXT NOT NULL,
    user_id      TEXT,
    thread_ts    TEXT,
    text         TEXT,
    rendered     TEXT,
    subtype      TEXT,
    reply_count  INTEGER DEFAULT 0,
    is_edited    INTEGER NOT NULL DEFAULT 0,
    is_deleted   INTEGER NOT NULL DEFAULT 0,
    raw_json     TEXT,
    synced_at    INTEGER NOT NULL,
    PRIMARY KEY (channel_id, ts)
) WITHOUT ROWID;
CREATE INDEX idx_messages_thread ON messages(channel_id, thread_ts) WHERE thread_ts IS NOT NULL;
CREATE INDEX idx_messages_user ON messages(user_id);
CREATE INDEX idx_messages_time ON messages(ts);

-- Reactions
CREATE TABLE reactions (
    channel_id   TEXT NOT NULL,
    message_ts   TEXT NOT NULL,
    emoji        TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    synced_at    INTEGER NOT NULL,
    PRIMARY KEY (channel_id, message_ts, emoji, user_id),
    FOREIGN KEY (channel_id, message_ts) REFERENCES messages(channel_id, ts)
) WITHOUT ROWID;
CREATE INDEX idx_reactions_emoji ON reactions(emoji);

-- File attachments
CREATE TABLE files (
    id                   TEXT PRIMARY KEY,
    channel_id           TEXT,
    message_ts           TEXT,
    name                 TEXT,
    mimetype             TEXT,
    size_bytes           INTEGER,
    url_private          TEXT,
    url_private_download TEXT,
    local_path           TEXT,
    synced_at            INTEGER NOT NULL,
    FOREIGN KEY (channel_id, message_ts) REFERENCES messages(channel_id, ts)
);

-- Channel membership
CREATE TABLE channel_members (
    channel_id   TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    PRIMARY KEY (channel_id, user_id),
    FOREIGN KEY (channel_id) REFERENCES channels(id)
) WITHOUT ROWID;

-- Sync state per channel
CREATE TABLE sync_state (
    channel_id   TEXT PRIMARY KEY,
    oldest_ts    TEXT,
    newest_ts    TEXT,
    is_complete  INTEGER NOT NULL DEFAULT 0,
    last_sync    INTEGER NOT NULL,
    cursor       TEXT
);

-- Subscriptions
CREATE TABLE subscriptions (
    channel_id    TEXT PRIMARY KEY,
    channel_name  TEXT,
    subscribed_at INTEGER NOT NULL,
    retention_days INTEGER,
    sync_threads  INTEGER NOT NULL DEFAULT 1,
    sync_members  INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (channel_id) REFERENCES channels(id)
);

-- Saved items
CREATE TABLE saved_items (
    channel_id   TEXT NOT NULL,
    message_ts   TEXT NOT NULL,
    saved_at     INTEGER NOT NULL,
    PRIMARY KEY (channel_id, message_ts)
) WITHOUT ROWID;

-- Pins
CREATE TABLE pins (
    channel_id   TEXT NOT NULL,
    message_ts   TEXT NOT NULL,
    pinned_by    TEXT,
    pinned_at    INTEGER,
    PRIMARY KEY (channel_id, message_ts)
) WITHOUT ROWID;

-- Shadow rowid table for FTS5 (messages uses WITHOUT ROWID)
CREATE TABLE messages_rowid_map (
    rowid      INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id TEXT NOT NULL,
    ts         TEXT NOT NULL,
    UNIQUE(channel_id, ts)
);

-- FTS5 virtual table
CREATE VIRTUAL TABLE messages_fts USING fts5(
    text,
    rendered,
    content = '',
    content_rowid = 'rowid'
);

-- Trigger: on message insert
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
    INSERT OR IGNORE INTO messages_rowid_map(channel_id, ts) VALUES (NEW.channel_id, NEW.ts);
    INSERT INTO messages_fts(rowid, text, rendered)
        SELECT rowid, NEW.text, NEW.rendered FROM messages_rowid_map
        WHERE channel_id = NEW.channel_id AND ts = NEW.ts;
END;

-- Trigger: on message update (contentless FTS5: delete old entry, insert new)
CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, text, rendered)
        SELECT 'delete', rowid, OLD.text, OLD.rendered FROM messages_rowid_map
        WHERE channel_id = OLD.channel_id AND ts = OLD.ts;
    INSERT INTO messages_fts(rowid, text, rendered)
        SELECT rowid, NEW.text, NEW.rendered FROM messages_rowid_map
        WHERE channel_id = NEW.channel_id AND ts = NEW.ts;
END;

-- Trigger: on message delete (contentless FTS5: special delete syntax)
CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, text, rendered)
        SELECT 'delete', rowid, OLD.text, OLD.rendered FROM messages_rowid_map
        WHERE channel_id = OLD.channel_id AND ts = OLD.ts;
    DELETE FROM messages_rowid_map WHERE channel_id = OLD.channel_id AND ts = OLD.ts;
END;
"#;
