# slackers User Guide

A Slack CLI written in Rust. All output is compact JSON — pipe it to `jq` for filtering and formatting.

---

## Table of Contents

1. [Installation](#installation)
2. [Authentication](#authentication)
3. [Configuration](#configuration)
4. [Messages](#messages)
5. [Channel History & DM Downloads](#channel-history--dm-downloads)
6. [Search](#search)
7. [Channels](#channels)
8. [Users](#users)
9. [Canvas Documents](#canvas-documents)
10. [Tips](#tips)

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

Create `~/.config/slackers/config.toml` (Linux) or `~/Library/Application Support/slackers/config.toml` (macOS) to control `message history` behavior:

```toml
[history]
# Resume interrupted history runs automatically (default: true)
auto_resume = true

# Drop system messages by subtype
exclude_subtypes = ["channel_join", "channel_leave", "channel_topic", "channel_purpose", "bot_message"]

# Drop messages from specific users by ID
exclude_users = ["USLACKBOT"]
```

**Cache files** (managed automatically, no need to edit):
- `channel-cache.json` — channel name → ID cache (avoids re-paginating on each run)
- `history-cursors.json` — resume checkpoints for interrupted `message history` runs

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

If the message is the root of a thread, the output includes a `thread_summary` with the reply count. File attachments are downloaded automatically to the current directory.

Options:
- `--max-body-chars <N>` — truncate message body to N characters (default 8000, `-1` for unlimited)
- `--include-reactions` — add reactions and reacting users to output
- `--workspace <url>` — target a specific workspace

### List a full thread

```bash
# From a thread URL
slackers message list https://yourteam.slack.com/archives/C123ABC/p1234567890123456?thread_ts=...

# From a channel + thread root ts
slackers message list "#general" --thread-ts 1234567890.123456
```

Filtering options:
- `--limit <N>` — return at most N messages
- `--user "@alice"` — only messages from a specific user
- `--has-link` — only messages that contain links
- `--has-file` — only messages with file attachments
- `--has-reaction` — only messages that have reactions
- `--after-ts <ts|YYYY-MM-DD>` — messages after this timestamp
- `--before-ts <ts|YYYY-MM-DD>` — messages before this timestamp
- `--max-body-chars <N>` — truncate body (default 8000, `-1` for unlimited)
- `--include-reactions` — include reaction data

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
```

### Add / remove reactions

```bash
# Add a reaction (emoji formats: :rocket:, rocket, or 🚀)
slackers message react add <message-url> :thumbsup:
slackers message react add C123ABC ":tada:" --ts 1234567890.123456

# Remove a reaction
slackers message react remove <message-url> :thumbsup:
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
      "thread": [...]   // present only with --include-threads
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
| `--max-content-chars <N>` | Truncate content (default 4000, `-1` unlimited) |
| `--workspace <url>` | Target a specific workspace |

**Example:** find all messages mentioning "incident" in #ops from January 2025:

```bash
slackers search messages "incident" \
  --channel "#ops" \
  --after 2025-01-01 \
  --before 2025-02-01 \
  --limit 100
```

---

## Channels

### List channels

```bash
# Public and private channels (default)
slackers channel list

# Filter by type
slackers channel list --types public_channel
slackers channel list --types private_channel
slackers channel list --types im          # direct messages
slackers channel list --types mpim        # group direct messages

# Include archived channels
slackers channel list --exclude-archived false

# Include member count
slackers channel list --include-num-members

# Larger result set
slackers channel list --limit 1000
```

Channel types can be combined by repeating `--types`:

```bash
slackers channel list --types public_channel --types private_channel
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
slackers canvas get <url> --max-chars -1
```

The default truncation limit is 20,000 characters.

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

### Credentials storage

Credentials are stored at:
- **macOS:** `~/Library/Application Support/slackers/credentials.json`
- **Linux:** `~/.config/slackers/credentials.json`

Tokens are stored in plaintext. Restrict access: `chmod 600 <credentials-file>`.

`auth whoami` shows all configured workspaces with tokens redacted (first 6, last 4 characters only).
