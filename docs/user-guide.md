# slackers User Guide

A Slack CLI written in Rust. All output is compact JSON — pipe it to `jq` for filtering and formatting. Also ships an MCP server for AI agent integration.

---

## Table of Contents

1. [Installation](#installation)
2. [Authentication](#authentication)
3. [Configuration](#configuration)
4. [Global Flags](#global-flags)
5. [Messages](#messages)
6. [Channel History & DM Downloads](#channel-history--dm-downloads)
7. [Search](#search)
8. [Unreads](#unreads)
9. [Channels](#channels)
10. [Users](#users)
11. [Canvas Documents](#canvas-documents)
12. [Direct Messages](#direct-messages)
13. [Batch Operations](#batch-operations)
14. [Files](#files)
15. [Saved Items (Later)](#saved-items-later)
16. [Scheduled Messages](#scheduled-messages)
17. [Mentions](#mentions)
18. [Export](#export)
19. [Workflows](#workflows)
20. [Slash Commands](#slash-commands)
21. [MCP Server](#mcp-server)
22. [Local Store](#local-store)
23. [Sync](#sync)
24. [Query](#query)
25. [Report](#report)
26. [Watch](#watch)
27. [Output Format](#output-format)
28. [Error Handling](#error-handling)
29. [Tips](#tips)

---

## Installation

### From source

```bash
git clone https://github.com/sthadka/slackers.git
cd slackers
cargo build --release
# binary is at target/release/slackers
```

**Requirements:** Rust 1.70+. macOS required for `import-desktop` and `import-chrome`.

---

## Authentication

slackers supports two token types:

| Type | Tokens | Notes |
|------|--------|-------|
| Standard | `xoxb-...` or `xoxp-...` | Bot or user token |
| Browser | `xoxc-...` + `xoxd-...` cookie | Full user-scoped access |

### Add credentials manually

```bash
# Standard token
slackers auth add \
  --workspace-url https://yourteam.slack.com \
  --token xoxb-your-token

# Browser tokens (xoxc + xoxd cookie)
slackers auth add \
  --workspace-url https://yourteam.slack.com \
  --xoxc xoxc-your-token \
  --xoxd xoxd-your-cookie
```

### Import from Slack Desktop (macOS)

Imports all logged-in workspaces directly from Slack Desktop's local storage — no need to find tokens manually.

```bash
slackers auth import-desktop
```

### Import from Chrome (macOS)

Reads credentials from an open Slack tab in Google Chrome using AppleScript.

```bash
slackers auth import-chrome
```

### Import from a cURL command

Copy a Slack API request as cURL (from browser DevTools → right-click request → Copy as cURL), then paste it into stdin:

```bash
slackers auth parse-curl
# paste the curl command, then Ctrl-D
```

### Environment variables

```bash
export SLACK_TOKEN=xoxb-your-token           # standard token
export SLACK_TOKEN=xoxc-your-token           # browser token
export SLACK_COOKIE_D=xoxd-your-cookie       # required with xoxc
```

### Managing workspaces

```bash
slackers auth whoami              # list all configured workspaces (tokens redacted)
slackers auth test                # verify the default workspace credentials
slackers auth test --workspace https://yourteam.slack.com   # test a specific workspace
slackers auth set-default https://yourteam.slack.com
slackers auth remove https://yourteam.slack.com
```

When you have multiple workspaces, most commands accept `--workspace <url>` to target a specific one. Without it, slackers uses the default.

---

## Configuration

Generate a default config file with all options documented:

```bash
slackers config init
```

This creates `~/.config/slackers/config.toml` (Linux) or `~/Library/Application Support/slackers/config.toml` (macOS). To see the path without creating it:

```bash
slackers config path
```

The config file controls history, store, and sync behavior:

```toml
[history]
# Resume interrupted history runs automatically (default: true)
auto_resume = true

# Drop system messages by subtype
exclude_subtypes = ["channel_join", "channel_leave", "channel_topic", "channel_purpose", "bot_message"]

# Drop messages from specific users by ID
exclude_users = ["USLACKBOT"]

[store]
enabled = true               # enable the local SQLite store (default: false)
sync_scope = "public"        # public | public+private | all | selected
store_raw_json = false       # keep full Slack JSON payloads
max_db_size_mb = 500         # warn/prune threshold, 0 = unlimited
auto_gc = true               # run gc before sync

[store.defaults]
retention_days = 90          # default for new subscriptions
sync_threads = true
sync_members = false
sync_files = false
max_file_size_mb = 10
```

**Cache files** (managed automatically, no need to edit):
- `channel-cache.json` — channel name → ID cache (avoids re-paginating on each run)
- `history-cursors.json` — resume checkpoints for interrupted `message history` runs

---

## Global Flags

These flags apply to all commands:

| Flag | Description |
|------|-------------|
| `--read-only` | Block all write operations (send, update, delete, pin, react, etc.) |
| `--pretty` | Produce indented JSON instead of compact single-line JSON |
| `--quiet` | Minimal JSON output for write operations (e.g. `{"ok":true}`) |
| `--no-progress` | Suppress spinner and progress bar output on stderr |
| `--local-only` | Force all reads to use the local store exclusively (errors if data is unavailable) |
| `--remote` | Force API calls, bypassing the local store |

---

## Messages

### Get a single message

```bash
# From a Slack message URL (copy link → "Copy link to message" in Slack)
slackers message get https://yourteam.slack.com/archives/C123ABC/p1234567890123456

# From a channel + timestamp
slackers message get "#general" --ts 1234567890.123456

# From a channel ID
slackers message get C123ABC --ts 1234567890.123456
```

If the message is the root of a thread, the output includes a `thread_summary` with the reply count. File attachments are downloaded automatically.

Options:
- `--max-body-chars <N>` — truncate message body to N characters (default 8000, `-1` for unlimited)
- `--include-reactions` — add reactions and reacting users to output
- `--resolve-users` — resolve user IDs to display names
- `--workspace <url>` — target a specific workspace

### List a full thread

```bash
# From a thread URL
slackers message list https://yourteam.slack.com/archives/C123ABC/p1234567890123456

# From a channel + thread root ts
slackers message list "#general" --thread-ts 1234567890.123456
```

Filtering options:
- `--limit <N>` — return at most N messages
- `--user "@alice"` — only messages from a specific user
- `--has-link` — only messages that contain links
- `--has-file` — only messages with file attachments
- `--has-reaction` — only messages that have reactions
- `--with-reaction <emoji>` — only messages with a specific reaction
- `--without-reaction <emoji>` — exclude messages with a specific reaction
- `--after-ts <ts>` — messages after this timestamp
- `--before-ts <ts>` — messages before this timestamp
- `--max-body-chars <N>` — truncate body (default 8000, `-1` for unlimited)
- `--include-reactions` — include reaction data
- `--resolve-users` — resolve user IDs to display names
- `--format` — output format (json/table/markdown/plain)

**Example:** first 10 messages from Alice that include files:

```bash
slackers message list <thread-url> --user "@alice" --has-file --limit 10
```

### Send a message

```bash
# To a channel
slackers message send "#general" "Hello from slackers!"

# Reply into a thread (thread_ts is the root message's timestamp)
slackers message send "#general" "Reply here" --thread-ts 1234567890.123456

# Reply to a specific message URL (automatically threads)
slackers message send https://yourteam.slack.com/archives/C123/p1234567890123456 "My reply"

# Broadcast a thread reply to the channel
slackers message send <url> "Also posting to channel" --reply-broadcast

# Send Block Kit blocks from a JSON file
slackers message send "#general" "Fallback text" --blocks blocks.json

# Send Block Kit blocks from stdin
echo '[{"type":"section","text":{"type":"mrkdwn","text":"*Bold*"}}]' | \
  slackers message send "#general" "Fallback" --blocks -
```

### Add / remove reactions

```bash
# Add a reaction
slackers message react add <message-url> thumbsup
slackers message react add C123ABC eyes --ts 1234567890.123456

# Remove a reaction
slackers message react remove <message-url> thumbsup
```

### Pin / unpin messages

```bash
slackers message pin --channel C123 --ts 1234567890.123456
slackers message unpin --channel C123 --ts 1234567890.123456
```

### Update / delete messages

```bash
slackers message update --channel C123 --ts 1234567890.123456 --text "corrected text"
slackers message delete --channel C123 --ts 1234567890.123456
```

### Thread participants

```bash
slackers message thread-participants <message-url>
slackers message thread-participants --channel C123 --ts 1234567890.123456 --resolve-users
```

---

## Channel History & DM Downloads

`message history` downloads the complete message history of any channel, DM, or group DM to a local JSON file. It writes incrementally — if interrupted, re-running the command resumes from where it left off.

### Download a public or private channel

```bash
# By channel name (writes to general-history.json)
slackers message history "#general"

# By channel ID (faster — skips name resolution)
slackers message history C123ABC

# Custom output file
slackers message history "#general" --output ~/exports/general.json
```

### Download a DM (direct message) conversation

DMs have channel IDs starting with `D`. They have no `name` field — instead, `user` contains the other person's Slack user ID. First list DMs to find the channel ID, then look up the user if you need to match a name:

```bash
# Step 1: list DM channels (shows id + user ID of the other person)
slackers channel list --types im | jq '.[] | {id, user}'

# Step 2: look up a user ID to confirm who it is
slackers user get U123ABC

# Step 3: download the conversation
slackers message history D123ABC
```

### Download a group DM (MPIM)

Group DMs have channel IDs starting with `G`. They also lack a `name` field; `user` is absent on MPIMs — use the `id` directly:

```bash
# Step 1: list group DMs
slackers channel list --types mpim | jq '.[] | {id}'

# Step 2: download
slackers message history G123ABC
```

### History options

| Flag | Default | Description |
|------|---------|-------------|
| `--limit <N>` | 500 | Max top-level messages to fetch |
| `--after <YYYY-MM-DD>` | — | Only messages after this date |
| `--before <YYYY-MM-DD>` | — | Only messages before this date |
| `--include-threads` | off | Inline full thread replies for every threaded message |
| `--include-reactions` | off | Include reactions on messages |
| `--max-body-chars <N>` | 8000 | Truncate message body (`-1` for unlimited) |
| `-o / --output <path>` | `<channel>-history.json` | Output file path |

### Export a year of history with threads

```bash
slackers message history "#engineering" \
  --limit 10000 \
  --after 2024-01-01 \
  --before 2025-01-01 \
  --include-threads \
  --include-reactions \
  --max-body-chars -1 \
  --output engineering-2024.json
```

### Output format

The JSON file is written after each fetched page and updated incrementally:

```json
{
  "channel": "#general",
  "channel_id": "C123ABC",
  "message_count": 842,
  "messages": [
    {
      "ts": "1234567890.123456",
      "user": "U123ABC",
      "text": "Hello!",
      "reply_count": 3,
      "thread": [...]
    }
  ]
}
```

### Resume an interrupted run

Auto-resume is on by default. Just re-run the same command with the same output file:

```bash
slackers message history "#general" --output general.json
# interrupted... re-run:
slackers message history "#general" --output general.json
# picks up from the oldest message already in the file
```

To disable auto-resume, set `auto_resume = false` in `config.toml`.

---

## Search

Search across messages and files in the workspace. Requires a user token (`xoxp-`) or browser token (`xoxc-`/`xoxd-`).

```bash
# Search everything
slackers search all "deployment failed"

# Messages only
slackers search messages "out of memory" --channel "#logs" --after 2024-06-01

# Files only
slackers search files "budget" --content-type image
```

### Search options

| Flag | Description |
|------|-------------|
| `--channel <name/id>` | Filter by channel (repeatable for multiple) |
| `--user <@name/id>` | Filter by message author |
| `--after <YYYY-MM-DD>` | Results after this date |
| `--before <YYYY-MM-DD>` | Results before this date |
| `--content-type <type>` | `any`, `text`, `image`, `snippet`, `file` |
| `--limit <N>` | Max results (default 20, max 200) |
| `--max-body-chars <N>` | Truncate content (default 4000, `-1` unlimited) |
| `--sort <field>` | Sort by `timestamp` or `relevance` |
| `--has-link` | Only results containing links |
| `--has-emoji` | Only results containing emoji |
| `--from-me` | Only results from the authenticated user |
| `--resolve-users` | Resolve user IDs to display names |
| `--format` | Output format (json/table/markdown/plain) |
| `--workspace <url>` | Target a specific workspace |
| `--highlight` | Highlight matched terms in FTS5 local search results |
| `--all-channels` | Search all subscribed channels (local FTS5, ignore --channel filter) |
| `--regex <pattern>` | Post-filter results with a regex pattern on the text field |

**Example:** find all messages mentioning "incident" in #ops from January 2025:

```bash
slackers search messages "incident" \
  --channel "#ops" \
  --after 2025-01-01 \
  --before 2025-02-01 \
  --limit 100
```

---

## Unreads

Show unread messages across all your conversations — channels, DMs, and threads.

```bash
# Full unread summary with message previews
slackers unreads show

# Just the counts (faster)
slackers unreads show --counts-only

# Control preview depth
slackers unreads show --max-messages 5 --max-body-chars 2000

# Exclude system messages (join/leave/topic changes)
slackers unreads show --include-system false
```

### Unreads options

| Flag | Default | Description |
|------|---------|-------------|
| `--counts-only` | off | Show only unread counts, no message content |
| `--max-messages <N>` | 10 | Max unread messages to preview per conversation |
| `--max-body-chars <N>` | 4000 | Truncate message body (`-1` for unlimited) |
| `--include-system` | off | Include system messages (join, leave, topic changes) |
| `--format` | json | Output format (json/table/markdown/plain) |
| `--workspace <url>` | — | Target a specific workspace |

---

## Channels

### List channels

```bash
# Public and private channels (default)
slackers channel list

# All channels including ones you haven't joined
slackers channel list --all

# Filter by type
slackers channel list --types public_channel
slackers channel list --types private_channel
slackers channel list --types im          # direct messages
slackers channel list --types mpim        # group direct messages

# Combine types
slackers channel list --types public_channel --types private_channel

# Resolve DM user IDs to display names
slackers channel list --types im --resolve-users

# Larger result set
slackers channel list --limit 1000

# Output as table
slackers channel list --format table
```

### Get channel info

```bash
slackers channel get "#general"
slackers channel get C123ABC
slackers channel get C123ABC --include-num-members
```

### Join / leave a channel

```bash
slackers channel join "#project-alpha"
slackers channel leave "#project-alpha"
```

### Create a channel

```bash
# Public channel
slackers channel new --name "project-beta"

# Private channel
slackers channel new --name "project-beta" --private
```

### Rename a channel

```bash
slackers channel rename "#old-name" "new-name"
slackers channel rename C123ABC "new-name"
```

### Invite users

```bash
slackers channel invite "#project-alpha" --users U123,U456,U789
```

### List channel members

```bash
slackers channel members "#general"
slackers channel members C123ABC --resolve-users
```

### Mark as read

```bash
slackers channel mark "#general" --ts 1234567890.123456
```

**Performance tip:** channel IDs (e.g. `C0123456789`) are always faster than names. slackers caches name→ID lookups in `channel-cache.json` after the first resolution, so subsequent runs are fast.

---

## Users

```bash
# List users (bots excluded by default)
slackers user list

# Include bots
slackers user list --include-bots

# Paginate large workspaces
slackers user list --limit 500 --cursor <cursor-from-previous-response>

# Output as table
slackers user list --format table

# Get a specific user
slackers user get U123ABC
slackers user get @alice
```

---

## Canvas Documents

Fetch a Slack canvas and get it as Markdown:

```bash
# From a canvas URL
slackers canvas get https://yourteam.slack.com/docs/T123/F456ABC

# From a canvas/file ID
slackers canvas get F456ABC --workspace https://yourteam.slack.com

# Unlimited content
slackers canvas get <url> --max-body-chars -1
```

The default truncation limit is 20,000 characters.

---

## Direct Messages

```bash
# Open a DM (or MPIM) conversation — returns the channel ID
slackers dm open --users U123

# Open a group DM
slackers dm open --users U123,U456,U789

# Open and send in one step
slackers dm send --users U123 --message "Hey, can you review this?"
```

---

## Batch Operations

Send messages or reactions to multiple targets at once.

```bash
# Send the same message to multiple channels
slackers batch send --message "Maintenance tonight 22:00 UTC" --channels "#general,#ops,#engineering"

# Add a reaction to multiple messages
slackers batch react --emoji rocket --messages "https://...url1,https://...url2"
```

---

## Files

```bash
# Upload a file
slackers file upload --file ./report.pdf --channels "#general" --title "Q2 Report"
slackers file upload --file ./data.csv --channels C123,C456 --comment "Updated data"

# List files in a channel
slackers file list --channel C123 --limit 50

# List all files in the workspace
slackers file list

# Delete a file
slackers file delete --file-id F123ABC
```

---

## Saved Items (Later)

Star or save messages for later review.

```bash
# Save a message
slackers later add --channel C123 --ts 1234567890.123456

# List saved items
slackers later list
slackers later list --limit 50

# Remove from saved
slackers later remove --channel C123 --ts 1234567890.123456
```

---

## Scheduled Messages

Schedule messages for future delivery.

```bash
# Schedule a message (RFC 3339 timestamp)
slackers scheduled send --channel C123 --message "Reminder: standup in 5 min" --at "2026-12-31T09:00:00Z"

# Schedule with unix timestamp
slackers scheduled send --channel C123 --message "Happy New Year!" --at 1798761600

# List scheduled messages
slackers scheduled list
slackers scheduled list --channel C123

# Cancel a scheduled message
slackers scheduled delete --channel C123 --id Q123ABC
```

---

## Mentions

List messages that @mention you or another user.

```bash
# Your recent mentions
slackers mention list

# Mentions in a specific channel and date range
slackers mention list --channel "#general" --after 2026-01-01 --before 2026-07-01

# Another user's mentions
slackers mention list --username alice --limit 50
```

### Mention options

| Flag | Description |
|------|-------------|
| `--username <handle>` | Show mentions for this user instead of yourself |
| `--channel <name/id>` | Filter by channel (repeatable) |
| `--after <YYYY-MM-DD>` | Mentions after this date |
| `--before <YYYY-MM-DD>` | Mentions before this date |
| `--limit <N>` | Max results (default 20) |
| `--max-body-chars <N>` | Truncate body (default 4000) |
| `--workspace <url>` | Target a specific workspace |

---

## Export

Export full channel history in various formats.

```bash
# Export as JSON (default)
slackers export channel --channel "#general"

# Export as CSV
slackers export channel --channel "#general" --format csv --output general.csv

# Export as HTML
slackers export channel --channel C123ABC --format html --output general.html
```

---

## Workflows

Discover and run Slack Workflow Builder workflows.

```bash
# List workflows bookmarked or featured in a channel
slackers workflow list "#engineering"

# Preview workflow metadata from a trigger ID
slackers workflow preview Ft123ABC

# Get full workflow definition (form fields, steps)
slackers workflow get Ft123ABC
slackers workflow get Wf123ABC

# Run a workflow trigger
slackers workflow run Ft123ABC --channel "#engineering"
```

---

## Slash Commands

Execute slash commands programmatically (requires browser token `xoxc-`).

```bash
slackers slash run --channel "#general" /remind me "standup" every weekday at 9am
slackers slash run --channel C123 /poll "Lunch?" "Pizza" "Sushi" "Tacos"
```

---

## MCP Server

`slackers serve` starts a Model Context Protocol server over stdio, exposing all CLI commands as MCP tools. This lets AI agents call Slack operations directly.

```bash
slackers serve
```

### Configuration

Add to your MCP client config (e.g. Claude Desktop `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "slack": {
      "command": "slackers",
      "args": ["serve"]
    }
  }
}
```

For Claude Code (`.claude/settings.json` or global settings):

```json
{
  "mcpServers": {
    "slack": {
      "command": "slackers",
      "args": ["serve"]
    }
  }
}
```

For read-only access (write tools hidden):

```json
{
  "mcpServers": {
    "slack": {
      "command": "slackers",
      "args": ["--read-only", "serve"]
    }
  }
}
```

### Available tools

The server exposes 45+ tools organized by category:

| Category | Read tools | Write tools |
|----------|-----------|-------------|
| Messages | `message_get`, `message_list`, `message_history`, `message_thread_participants` | `message_send`, `message_update`, `message_delete`, `message_pin`, `message_unpin` |
| Reactions | — | `react_add`, `react_remove` |
| Search | `search_messages`, `search_files`, `search_all` | — |
| Channels | `channel_list`, `channel_get`, `channel_members` | `channel_join`, `channel_leave`, `channel_mark`, `channel_new`, `channel_invite`, `channel_rename` |
| Users | `user_list`, `user_get` | — |
| Canvas | `canvas_get` | — |
| Files | `file_list` | `file_upload`, `file_delete` |
| DMs | `dm_open` | `dm_send` |
| Batch | — | `batch_send`, `batch_react` |
| Saved | `later_list` | `later_add`, `later_remove` |
| Scheduled | `scheduled_list` | `scheduled_send`, `scheduled_delete` |
| Mentions | `mention_list` | — |
| Export | `export_channel` | — |
| Unreads | `unreads_show` | — |
| Workflows | `workflow_list`, `workflow_preview`, `workflow_get` | `workflow_run` |
| Slash | — | `slash_run` |
| Workspace | `workspace_info`, `emoji_list` | — |
| Store | `store_search`, `store_query_messages`, `store_query_threads`, `store_channel_summary`, `store_user_context`, `store_sync_status` | — |

---

## Local Store

slackers can maintain a local SQLite database of your Slack data for offline access, fast full-text search, and analytics. The database is stored at:

- **macOS:** `~/Library/Application Support/slackers/store-{hash}.db`
- **Linux:** `~/.local/share/slackers/store-{hash}.db`

The `{hash}` suffix is derived from your workspace, so each workspace gets its own database.

### Enable the store

Set `enabled = true` in the `[store]` section of your `config.toml`:

```toml
[store]
enabled = true
```

See the [Configuration](#configuration) section for the full set of store options.

### Store info

```bash
# Show database stats (size, message count, subscriptions, last sync time)
slackers store info
```

### Garbage collection

```bash
# Run retention cleanup and vacuum
slackers store gc
```

This removes messages older than the per-subscription `retention_days` setting and runs SQLite VACUUM to reclaim disk space. Runs automatically before each sync when `auto_gc = true`.

### Reset the store

```bash
# Drop and recreate all tables (destructive — requires confirmation)
slackers store reset
```

You must type `yes` to confirm. Blocked by `--read-only`.

### Subscriptions

Subscriptions control which channels are synced to the local store. Each subscription can have its own retention and sync settings.

```bash
# Subscribe to a channel
slackers store sub add "#engineering"

# Subscribe with custom retention and options
slackers store sub add "#general" --retention 30d --no-threads --with-files --with-members

# Subscribe to a DM
slackers store sub add --dm @alice

# Subscribe to channels matching a glob pattern
slackers store sub add --pattern "eng-*"

# Unsubscribe from a channel
slackers store sub remove "#engineering"

# List all subscriptions with sync state
slackers store sub list
```

### Export and import

```bash
# Export store data as JSON (default)
slackers store export

# Export a specific channel as CSV
slackers store export --format csv --channel "#general" --output general.csv

# Import previously exported data
slackers store import --file export.json
```

---

## Sync

Sync populates the local store with data from the Slack API. Requires the store to be enabled.

### Real-time sync

```bash
# Start real-time sync (foreground)
slackers sync start

# Start as a background daemon
slackers sync start --daemon

# Set polling interval in seconds (for bot tokens)
slackers sync start --daemon --interval 60
```

Browser tokens (`xoxc-`) use WebSocket for real-time updates. Bot tokens (`xoxb-`) use polling at the configured interval.

### Stop and status

```bash
# Stop the sync daemon
slackers sync stop

# Show sync state (running/stopped, last sync time, channels syncing)
slackers sync status
```

### One-shot sync

```bash
# Backfill all subscribed channels via REST (full history catch-up)
slackers sync backfill

# Fetch only the latest messages since the last sync
slackers sync once
```

---

## Query

Query the local store with structured filters. All query subcommands read from the local SQLite database and do not make Slack API calls.

### Messages

```bash
# Recent messages from a user in a channel
slackers query messages --user "@alice" --channel "#engineering" --after 7d --limit 50

# Messages sorted by reply count
slackers query messages --channel "#general" --sort replies --limit 20

# Messages in a time range
slackers query messages --after "2026-01-01" --before "2026-02-01"
```

| Flag | Description |
|------|-------------|
| `--user <@name/id>` | Filter by message author |
| `--channel <#name/id>` | Filter by channel |
| `--after <relative or timestamp>` | Messages after this point (e.g. `7d`, `2026-01-01`) |
| `--before <relative or timestamp>` | Messages before this point |
| `--text <substring>` | Filter by text content |
| `--sort <field>` | Sort by `timestamp` or `replies` |
| `--limit <N>` | Max results |

### Threads

```bash
# Longest threads in a channel over the past 30 days
slackers query threads --channel "#engineering" --after 30d --sort duration --limit 10

# Most active threads by reply count
slackers query threads --sort replies --limit 20
```

| Flag | Description |
|------|-------------|
| `--user <@name/id>` | Filter by thread starter |
| `--channel <#name/id>` | Filter by channel |
| `--after <relative or timestamp>` | Threads after this point |
| `--before <relative or timestamp>` | Threads before this point |
| `--sort <field>` | Sort by `replies`, `participants`, or `duration` |
| `--limit <N>` | Max results |

### Reactions

```bash
# Most-used reactions in a channel
slackers query reactions --channel "#general" --group-by emoji --limit 10

# Reactions given by a specific user
slackers query reactions --user "@alice" --group-by user
```

| Flag | Description |
|------|-------------|
| `--channel <#name/id>` | Filter by channel |
| `--user <@name/id>` | Filter by user who reacted |
| `--emoji <name>` | Filter by specific emoji (without colons) |
| `--group-by <field>` | Group by `emoji` or `user` |
| `--limit <N>` | Max results |

### Files

```bash
# Largest files in a channel
slackers query files --channel "#design" --sort size --limit 20

# Search files by name or type
slackers query files --text "report" --sort name
```

| Flag | Description |
|------|-------------|
| `--channel <#name/id>` | Filter by channel |
| `--text <substring>` | Filter by file name or type |
| `--sort <field>` | Sort by `size` or `name` |
| `--limit <N>` | Max results |

### Activity

```bash
# Activity summary for the past week
slackers query activity --after 7d

# Activity in a date range
slackers query activity --after "2026-07-01" --before "2026-08-01" --limit 50
```

| Flag | Description |
|------|-------------|
| `--after <relative or timestamp>` | Activity after this point |
| `--before <relative or timestamp>` | Activity before this point |
| `--limit <N>` | Max results |

---

## Report

Generate analytics reports from the local store. All reports require the store to be enabled and populated via sync.

### Channel activity

```bash
# Activity report: messages/day, unique posters, thread ratio
slackers report activity --channel "#engineering" --period 30d
```

### User activity

```bash
# User report: message count, channels active, threads participated
slackers report user --user "@alice" --period 30d
```

### Thread analytics

```bash
# Thread report: total threads, longest, most-replied
slackers report threads --channel "#engineering" --period 30d
```

### Reaction analytics

```bash
# Reaction report: total reactions, most-used emoji
slackers report reactions --channel "#general"
slackers report reactions --channel "#general" --period 7d
```

### Mention analytics

```bash
# Mention report: total mentions, who mentions whom
slackers report mentions --user "@alice" --period 30d
```

---

## Watch

Stream new messages to stdout as they arrive. Requires the sync daemon to be running or the local store to be populated.

```bash
# Watch a single channel
slackers watch "#general"

# Watch multiple channels
slackers watch "#general" "#engineering" "#ops"

# Filter by user
slackers watch "#general" --user "@alice"

# Only messages containing links
slackers watch "#engineering" --has-link

# Output as JSON (default) or plain text
slackers watch "#general" --format json
slackers watch "#general" --format plain
```

---

## Output Format

All commands output compact JSON by default. Use `--pretty` for indented JSON, or `--quiet` for minimal write-operation output.

Many list/tabular commands support `--format`:

| Value | Aliases | Description |
|-------|---------|-------------|
| `json` | — | Pretty-printed JSON (default) |
| `table` | — | ASCII table (comfy-table) |
| `markdown` | `md` | GitHub-flavored Markdown table |
| `plain` | `text` | Tab-separated or key=value lines |

```bash
slackers channel list --format table
slackers search messages "deploy" --format markdown
slackers message list <url> --format plain
```

---

## Error Handling

Errors are returned as structured JSON with typed exit codes:

| Exit Code | Meaning |
|-----------|---------|
| 0 | Success |
| 1 | General error |
| 3 | Authentication error |
| 4 | Network error |
| 5 | Slack API error |

```json
{
  "error": "auth_failed",
  "type": "auth",
  "message": "Token has been revoked",
  "retryable": false
}
```

---

## Tips

### Use channel IDs for speed

Name resolution paginates the full channel list, which is slow on large workspaces. Run `channel list` once to find IDs, then use IDs directly in all subsequent commands. The name→ID cache (`channel-cache.json`) makes repeat name lookups fast automatically.

```bash
# Find ID once
slackers channel list | jq '.[] | select(.name == "general") | .id'
# C0123456789

# Use ID in future commands
slackers message history C0123456789
```

### Pipe output to jq

All output is JSON, designed for `jq`:

```bash
# Pretty-print
slackers channel list | jq .

# Extract just names and IDs
slackers channel list | jq '.[] | {id, name}'

# Find messages with files
slackers message list <url> --has-file | jq '.messages[] | {ts, user, text}'
```

### Multi-workspace workflows

When you have credentials for multiple workspaces, pass `--workspace` to target a specific one:

```bash
slackers message history "#general" --workspace https://workspace-a.slack.com
slackers message history "#general" --workspace https://workspace-b.slack.com
```

### Read-only mode for safety

Use `--read-only` to prevent accidental writes during exploration:

```bash
slackers --read-only message list <url>     # works
slackers --read-only message send "#x" "y"  # blocked
```

### Credentials storage

Credentials are stored at:
- **macOS:** `~/Library/Application Support/slackers/credentials.json`
- **Linux:** `~/.config/slackers/credentials.json`

Tokens are stored in plaintext. Restrict access: `chmod 600 <credentials-file>`.

`auth whoami` shows all configured workspaces with tokens redacted (first 6, last 4 characters only).
