# Feature: `message history` subcommand

## Goal

Add a `slackers message history <channel>` command that downloads all messages from a
channel — including full thread replies — suitable for bulk analysis by an AI agent.

## Background / Why This Is Needed

The existing commands fall short for full channel dumps:

- `slackers search messages` uses the Slack search API. It is limited in total results,
  doesn't guarantee chronological completeness, and doesn't include thread replies.
- `slackers message list` (when given a `#channel` target) requires `--thread-ts` because
  it calls `conversations.replies` — it fetches a single thread, not channel history.

There is already a `list_channel_messages` function in `src/slack/messages.rs` (line 193)
that calls `conversations.history` with full pagination, and a `ListMessagesOptions` struct
(line 167). Both are marked `#[allow(dead_code)]` and are not wired up to any CLI command.

## What to Build

### CLI

```
slackers message history <CHANNEL> [OPTIONS]

Arguments:
  <CHANNEL>   Channel name (#name) or ID (C...)

Options:
  --workspace <WORKSPACE>       Workspace URL (needed for multiple workspaces)
  --limit <LIMIT>               Max top-level messages to fetch [default: 500]
  --after <AFTER>               Only messages after this date (YYYY-MM-DD)
  --before <BEFORE>             Only messages before this date (YYYY-MM-DD)
  --max-body-chars <N>          Max chars per message body (-1 for unlimited) [default: 8000]
  --include-threads             Fetch and inline full thread replies for threaded messages
  --include-reactions           Include reactions on messages (and replies if --include-threads)
```

### Output format

```json
{
  "channel": "#channel-name",
  "message_count": 42,
  "messages": [
    {
      "ts": "1741234567.123456",
      "user": "U0123456789",
      "text": "Top-level message text",
      "reply_count": 3,
      "thread": [
        { "ts": "1741234568.000001", "user": "U9876543210", "text": "Reply 1" },
        { "ts": "1741234569.000002", "user": "U0123456789", "text": "Reply 2" }
      ]
    },
    {
      "ts": "1741234600.000000",
      "user": "U1111111111",
      "text": "Another top-level message with no replies"
    }
  ]
}
```

- `thread` is only present when `--include-threads` is set and `reply_count > 0`.
- The root message is excluded from the `thread` array (replies only).
- Each message uses the same compact format as the rest of the codebase
  (`to_compact_message` / `CompactSlackMessage`).

## Implementation Plan

### 1. `src/slack/messages.rs`

- Remove `#[allow(dead_code)]` from `ListMessagesOptions` (line 166) and
  `list_channel_messages` (line 192) so they can be exported and used.

### 2. `src/slack/mod.rs`

- Add to the `pub use messages::` line:
  `list_channel_messages, ListMessagesOptions`

### 3. `src/cli.rs`

Add `History` variant to `MessageCommand`:

```rust
/// Fetch all messages from a channel, with optional thread expansion
History {
    /// Channel name (#name) or ID
    channel: String,

    #[command(flatten)]
    options: MessageHistoryOptions,
},
```

Add `MessageHistoryOptions` struct:

```rust
#[derive(Args, Debug)]
pub struct MessageHistoryOptions {
    #[arg(long)]
    pub workspace: Option<String>,

    /// Max top-level messages to fetch (default: 500)
    #[arg(long, default_value = "500")]
    pub limit: usize,

    /// Only messages after YYYY-MM-DD
    #[arg(long)]
    pub after: Option<String>,

    /// Only messages before YYYY-MM-DD
    #[arg(long)]
    pub before: Option<String>,

    /// Max message body characters (-1 for unlimited, default 8000)
    #[arg(long, default_value = "8000")]
    pub max_body_chars: i32,

    /// Fetch and inline full thread replies for threaded messages
    #[arg(long)]
    pub include_threads: bool,

    /// Include reactions on messages
    #[arg(long)]
    pub include_reactions: bool,
}
```

### 4. `src/commands/message.rs`

Add a match arm in `handle_message`:

```rust
MessageCommand::History { channel, options } => {
    handle_message_history(&channel, options).await
}
```

Add the handler function. Key points:

- Use `resolve_auth` the same way as the other handlers.
- Convert `--after` / `--before` YYYY-MM-DD strings to Unix timestamp strings for the
  Slack API (`oldest` / `latest` params). Use `chrono` (already a dependency) with
  `NaiveDate::parse_from_str` + `.and_hms_opt(0, 0, 0)` + `.and_utc().timestamp()`.
  If the string already contains a `.` treat it as a raw ts and pass through unchanged.
- Call `list_channel_messages` with the converted timestamps and limit.
- For each message, call `to_compact_message` with `include_thread_ts: true`.
- If `--include-threads` and `reply_count > 0`: call `fetch_thread(&client, channel, ts)`,
  skip the first element (the root), compact the remaining replies, attach as `thread` key
  on the message JSON object.
- Output: `{ "channel": channel, "message_count": N, "messages": [...] }` via `to_json_output`.

## Notes

- `chrono` is already in `Cargo.toml` — no new dependencies needed.
- The `--include-threads` path will make one extra API call per threaded message. For
  channels with many threads this can be slow; that is acceptable for a bulk-analysis use case.
- The `channel` param is passed directly to the Slack API (it accepts `#name` or `C...` IDs).
- Match the style of the existing handlers: no unwrap, propagate errors with `?`,
  use `to_json_output` for output.
