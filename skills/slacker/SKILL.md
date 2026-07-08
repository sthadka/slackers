---
name: slacker
description: |
  Slack automation CLI and MCP server for AI agents. Use when:
  - Discovering channels in a workspace (list, search, get channel info, members)
  - Managing channels (join, leave, invite, create, rename)
  - Reading a Slack message or thread (given a URL or channel+ts)
  - Checking unread messages across all conversations
  - Downloading Slack attachments (snippets, images, files) to local paths
  - Searching Slack messages or files (advanced filters: user, date, has:link, etc.)
  - Sending a reply, DM, or batch messages; adding/removing reactions
  - Sending messages with Block Kit blocks (--blocks flag)
  - Fetching a Slack canvas as markdown
  - Looking up Slack users, listing workspace users
  - Uploading, listing, or deleting files
  - Fetching workspace info, custom emoji, saved items, scheduled messages
  - Monitoring mentions; exporting channel history
  - Running Slack Workflow Builder workflows or slash commands
  - Starting an MCP server to expose Slack tools to AI agents (slackers serve)
  - Formatting output as JSON, table, markdown, or plain text (--format flag)
  Triggers: "slack channel", "list channels", "join channel", "slack message", "slack thread", "slack URL", "slack link", "read slack", "reply on slack", "search slack", "slack DM", "slack file", "slack mention", "slack export", "slack workspace", "batch slack", "slack unread", "slack workflow", "slack MCP", "slackers serve"
---

# Slack automation with `slackers`

`slackers` is a CLI binary installed on `$PATH`. Invoke it directly (e.g. `slackers user list`)

## Quick start (auth)

Authentication is automatic on macOS (Slack Desktop first, then Chrome fallback).

If credentials aren't available, run one of:

- Slack Desktop (default):

```bash
slackers auth import-desktop
slackers auth test
```

- Chrome fallback:

```bash
slackers auth import-chrome
slackers auth test
```

- Or set env vars (browser tokens; avoid pasting these into chat logs):

```bash
export SLACK_TOKEN="xoxc-..."
export SLACK_COOKIE_D="xoxd-..."
slackers auth test
```

- Or set a standard token:

```bash
export SLACK_TOKEN="xoxb-..."  # or xoxp-...
slackers auth test
```

Check configured workspaces:

```bash
slackers auth whoami
```

## Canonical workflow (given a Slack message URL)

1. Fetch a single message (plus thread summary, if any):

```bash
slackers message get "https://workspace.slack.com/archives/C123/p1700000000000000"
```

2. If you need the full thread:

```bash
slackers message list "https://workspace.slack.com/archives/C123/p1700000000000000"
```

3. Filter thread messages:

```bash
# Only show messages from a specific user
slackers message list "https://workspace.slack.com/archives/C123/p1700000000000000" --user "@alice"

# Only show messages with file attachments
slackers message list "https://workspace.slack.com/archives/C123/p1700000000000000" --has-file

# Limit to first 10 messages
slackers message list "https://workspace.slack.com/archives/C123/p1700000000000000" --limit 10

# Combine filters: messages from alice with links
slackers message list "https://workspace.slack.com/archives/C123/p1700000000000000" --user "@alice" --has-link
```

## Attachments (snippets/images/files)

`message get/list` and `search` auto-download attachments and include absolute paths in JSON output (typically under `message.files[].path` / `files[].path`).

## Reply or react (does the right thing)

```bash
slackers message send "https://workspace.slack.com/archives/C123/p1700000000000000" "I can take this."
slackers message react add "https://workspace.slack.com/archives/C123/p1700000000000000" "eyes"
slackers message react remove "https://workspace.slack.com/archives/C123/p1700000000000000" "eyes"
```

Send with Block Kit blocks:

```bash
slackers message send "#general" "Fallback text" --blocks blocks.json
echo '[{"type":"section","text":{"type":"mrkdwn","text":"*Bold*"}}]' | slackers message send "#general" "Fallback" --blocks -
```

Broadcast a thread reply to the channel:

```bash
slackers message send "<thread-url>" "Also visible in channel" --reply-broadcast
```

## Search (messages + files)

Prefer channel-scoped search for reliability:

```bash
slackers search all "smoke tests failed" --channel "#alerts" --after 2026-01-01 --before 2026-02-01
slackers search messages "stably test" --user "@alice" --channel general
slackers search files "testing" --content-type snippet --limit 10
```

## Multi-workspace guardrail (important)

If you have multiple workspaces configured and you use a channel **name** (`#general` / `general`), pass `--workspace` (or set `SLACK_WORKSPACE_URL`) to avoid ambiguity:

```bash
slackers message get "#general" --workspace "https://myteam.slack.com" --ts "1770165109.628379"
```

## Unreads

```bash
slackers unreads show                           # unread messages across all conversations
slackers unreads show --counts-only             # just counts, no message content
slackers unreads show --max-messages 5 --format table
```

## Workflows

```bash
slackers workflow list "#engineering"           # workflows bookmarked in a channel
slackers workflow preview Ft123ABC              # preview workflow metadata
slackers workflow get Ft123ABC                  # full definition with form fields + steps
slackers workflow run Ft123ABC --channel "#engineering"  # run a workflow trigger
```

## Slash commands

Execute slash commands programmatically (requires browser token `xoxc-`):

```bash
slackers slash run --channel "#general" /remind me "standup" every weekday at 9am
```

## Channel Discovery

Discover and manage channels without prior knowledge:

```bash
# List all channels you have access to (returns IDs and names)
slackers channel list

# List only public channels
slackers channel list --types public_channel

# Get detailed channel information
# IMPORTANT: For best performance, use channel IDs from 'channel list'
slackers channel get "C0123456789" --include-num-members

# Using channel names works but requires scanning all channels (can hit rate limits)
slackers channel get "#general"

# Join a channel (accepts both IDs and names)
slackers channel join "#random"
slackers channel join "C0123456789"

# Leave a channel (accepts both IDs and names)
slackers channel leave "#random"
```

```bash
# Create a new channel
slackers channel new --name "project-beta" [--private]

# Rename a channel
slackers channel rename "#old-name" "new-name"
```

**Performance tip:** Channel IDs (like `C0123456789`) are faster than names because Slack's API requires ID lookup. Use `channel list` first to get IDs, then use those IDs in subsequent commands.

## Message management

```bash
# Pin / unpin / delete / update
slackers message pin --channel C123 --ts 1700000000.000001
slackers message delete --channel C123 --ts 1700000000.000001
slackers message update --channel C123 --ts 1700000000.000001 --text "corrected text"

# Thread participants
slackers message thread-participants "https://workspace.slack.com/archives/C123/p1700000000000000"

# Channel history (resumable, incremental)
slackers message history "#general" --limit 500 --after 2026-01-01 --include-threads
```

## Direct messages

```bash
slackers dm open --users U123,U456
slackers dm send --users U123 --message "Hey, can you review this?"
```

## Files

```bash
slackers file upload --file ./report.pdf --channels "#general" --title "Q2 Report"
slackers file list --channel C123 --limit 50
slackers file delete --file-id F123ABC
```

## Batch operations

```bash
# Send the same message to multiple channels
slackers batch send --message "Maintenance tonight 22:00 UTC" --channels "#general,#ops"

# Add a reaction to multiple messages
slackers batch react --emoji rocket --messages "https://...url1,https://...url2"
```

## Mentions

```bash
# Messages that @mention the authenticated user
slackers mention list --after 2026-01-01 --channel "#general"

# Mentions of a specific user
slackers mention list --username alice --limit 50
```

## Workspace / Emoji

```bash
slackers workspace info
slackers emoji list
```

## Later (starred/saved messages)

```bash
slackers later add --channel C123 --ts 1700000000.000001
slackers later list --limit 20
slackers later remove --channel C123 --ts 1700000000.000001
```

## Scheduled messages

```bash
slackers scheduled send --channel C123 --message "Reminder!" --at "2026-12-31T09:00:00Z"
slackers scheduled list
slackers scheduled delete --channel C123 --id Q123
```

## Export

```bash
slackers export channel --channel "#general" --format json --output general-export.json
slackers export channel --channel C123 --format csv
```

## Output format flag

Most list/tabular commands support `--format`:

```bash
slackers channel list --format table
slackers search messages "deploy" --format markdown
slackers message list <url> --format plain
```

Supported values: `json` (default), `table`, `markdown`/`md`, `plain`/`text`.

## Canvas + Users

```bash
slackers canvas get "https://workspace.slack.com/docs/T123/F456"
slackers user list --workspace "https://workspace.slack.com" --limit 100
slackers user get "@alice" --workspace "https://workspace.slack.com"
```

## MCP Server

`slackers serve` starts an MCP server over stdio, exposing 45+ tools to AI agents.

```bash
slackers serve
```

MCP client config (Claude Desktop, Claude Code, etc.):

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

Read-only mode (write tools hidden):

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

## Global flags

| Flag | Description |
|------|-------------|
| `--read-only` | Block all write operations |
| `--pretty` | Indented JSON output |
| `--quiet` | Minimal `{"ok":true}` for write ops |
| `--no-progress` | Suppress stderr spinners |

## References

- [references/commands.md](references/commands.md): full command map + all flags
- [references/targets.md](references/targets.md): URL vs `#channel` targeting rules
- [references/output.md](references/output.md): JSON output shapes + download paths
