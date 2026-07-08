# slackers

A comprehensive command-line interface for Slack workspace management and API operations, written in Rust. Also ships an **MCP server** so AI agents can call Slack tools over stdio.

## Overview

slackers is a full-featured Slack CLI that provides complete workspace management, message operations, search capabilities, and authentication handling. It offers a fast, reliable alternative to web-based Slack interactions with support for multiple workspaces and credential sources.

## Features

### MCP Server
- Start a Model Context Protocol server with `slackers serve`
- Exposes 45+ tools over JSON-RPC 2.0 / stdio
- Respects `--read-only` flag to filter out write tools
- Compatible with Claude Code, Claude Desktop, and any MCP-capable client

### Message Operations
- Fetch individual messages and entire threads
- Send messages to channels and threads with optional Block Kit (`--blocks`)
- Broadcast thread replies to the channel with `--reply-broadcast`
- Add and remove reactions
- Pin and unpin messages
- Update and delete messages
- List and filter thread messages with advanced criteria
- Download file attachments automatically
- Filter messages by user, links, files, reactions, or specific emoji
- Full channel history download with incremental resume

### Search
- Search messages across workspaces
- Search files with content type filtering
- Unified search (messages and files)
- Advanced query syntax with date ranges, user filters, and channel scoping
- Sort by timestamp or relevance

### Unreads
- Show unread messages across all conversations (channels, DMs, threads)
- Counts-only mode for quick triage
- Configurable message preview depth

### Workflow Automation
- List workflows bookmarked in a channel
- Preview workflow metadata from a trigger ID
- Inspect workflow definitions including form fields and steps
- Run workflow triggers programmatically

### Slash Commands
- Execute slash commands in a channel (requires browser token)

### Canvas Documents
- Fetch Slack canvas documents
- Convert to Markdown format
- Support for both URLs and file IDs
- Configurable content truncation

### User Management
- List workspace users with pagination
- Get user details by ID or @handle
- Filter active users and bots

### Channel Discovery & Management
- List all channels with type filtering (public, private, DMs, multi-party DMs)
- Get detailed channel information (topic, purpose, member count)
- Join, leave, rename channels
- Create new public or private channels
- Invite users to channels
- Mark channels as read
- List channel members with optional display name resolution

### Direct Messages
- Open DM or group DM conversations
- Send DMs in one step

### Batch Operations
- Send the same message to multiple channels at once
- Add reactions to multiple messages at once

### Files
- Upload files to channels with optional title and comment
- List files in a workspace or channel
- Delete files by ID

### Saved Items (Later)
- Star/save messages for later
- List and manage saved items

### Scheduled Messages
- Schedule messages for future delivery (unix timestamp or RFC 3339)
- List and cancel scheduled messages

### Mentions
- List messages that @mention you or a named user
- Filter by channel, date range

### Export
- Export full channel history to JSON, CSV, or HTML

### Authentication
- Multi-workspace credential management
- Standard (xoxb/xoxp) and browser (xoxc/xoxd) token support
- Import credentials from Slack Desktop (macOS)
- Import credentials from Chrome browser tabs (macOS)
- Parse credentials from cURL commands
- Environment variable support
- Secure storage in config directory

## Installation

### From Source

```bash
git clone https://github.com/sthadka/slackers.git
cd slackers
cargo build --release
```

The binary will be available at `target/release/slackers`.

### Prerequisites

- Rust 1.70 or higher
- macOS (for Desktop/Chrome import features)

## Quick Start

### Add Credentials

Standard token:
```bash
slackers auth add --workspace-url https://yourteam.slack.com --token xoxb-your-token
```

Browser tokens (for user-scoped operations):
```bash
slackers auth add --workspace-url https://yourteam.slack.com \
  --xoxc xoxc-your-token \
  --xoxd xoxd-your-cookie
```

Import from Slack Desktop (macOS):
```bash
slackers auth import-desktop
```

### Basic Usage

Test authentication:
```bash
slackers auth test
slackers auth whoami
```

Fetch a message:
```bash
slackers message get https://yourteam.slack.com/archives/C123/p1234567890
```

Send a message:
```bash
slackers message send "#general" "Hello from slackers!"
```

Search messages:
```bash
slackers search messages "error" --channel "#logs" --after 2024-01-01
```

List and join channels:
```bash
slackers channel list --types public_channel
slackers channel join "#random"
```

Check unreads:
```bash
slackers unreads show --counts-only
```

Start the MCP server:
```bash
slackers serve
```

## Global Flags

These flags apply to all commands:

| Flag | Description |
|------|-------------|
| `--read-only` | Block all write operations (send, update, delete, pin, react, etc.) |
| `--pretty` | Produce indented JSON instead of compact single-line JSON |
| `--quiet` | Minimal JSON output for write operations (e.g. `{"ok":true}`) |
| `--no-progress` | Suppress spinner and progress bar output on stderr |

## MCP Server

`slackers serve` starts a Model Context Protocol server over stdio, exposing all CLI commands as MCP tools. This lets AI agents (Claude Code, Claude Desktop, etc.) call Slack operations directly.

```bash
slackers serve
```

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

For read-only access:

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

The server exposes 45+ tools organized by category: messages, search, channels, users, files, canvas, batch, DMs, mentions, workflows, and more. Write tools are automatically hidden when `--read-only` is active.

## Configuration

Credentials are stored in:
- macOS: `~/Library/Application Support/slackers/credentials.json`
- Linux: `~/.config/slackers/credentials.json`

### Environment Variables

```bash
export SLACK_TOKEN=xoxb-your-token              # Standard token
export SLACK_TOKEN=xoxc-your-token              # Browser token (requires cookie)
export SLACK_COOKIE_D=xoxd-your-cookie          # Browser cookie
```

### History Configuration

Create `~/.config/slackers/config.toml` (Linux) or `~/Library/Application Support/slackers/config.toml` (macOS):

```toml
[history]
auto_resume = true
exclude_subtypes = ["channel_join", "channel_leave", "channel_topic", "channel_purpose", "bot_message"]
exclude_users = ["USLACKBOT"]
```

## Commands

### Authentication

```bash
slackers auth add              # Add workspace credentials
slackers auth whoami           # Show all configured workspaces
slackers auth test             # Test API connection
slackers auth set-default      # Set default workspace
slackers auth remove           # Remove workspace
slackers auth import-desktop   # Import from Slack Desktop (macOS)
slackers auth import-chrome    # Import from Chrome (macOS)
slackers auth parse-curl       # Parse cURL command from stdin
```

### Messages

```bash
slackers message get <url>                    # Get message or thread
slackers message list <target>                # List thread messages
slackers message send <target> <text>         # Send message
slackers message react add <target> <emoji>   # Add reaction
slackers message react remove <target> <emoji> # Remove reaction
slackers message history <channel>            # Download channel history
slackers message pin --channel C... --ts <ts> # Pin a message
slackers message unpin --channel C... --ts <ts>
slackers message update --channel C... --ts <ts> --text <text>
slackers message delete --channel C... --ts <ts>
slackers message thread-participants <url>    # Thread participant stats
```

### Search

```bash
slackers search all <query>                   # Search messages and files
slackers search messages <query>              # Search messages
slackers search files <query>                 # Search files
```

### Unreads

```bash
slackers unreads show                         # Show unread messages
slackers unreads show --counts-only           # Just unread counts
```

### Channels

```bash
slackers channel list                         # List all channels
slackers channel get <channel>                # Get channel info
slackers channel join <channel>               # Join a channel
slackers channel leave <channel>              # Leave a channel
slackers channel new --name <name>            # Create a channel
slackers channel rename <channel> <name>      # Rename a channel
slackers channel invite <channel> --users U1,U2
slackers channel members <channel>            # List members
slackers channel mark <channel> --ts <ts>     # Mark as read
```

### Canvas, Users, Files

```bash
slackers canvas get <url-or-id>               # Get canvas as Markdown
slackers user list                            # List all users
slackers user get <identifier>                # Get user details
slackers file upload --file <path> --channels C1,C2
slackers file list --channel C123
slackers file delete --file-id F123
```

### Direct Messages

```bash
slackers dm open --users U123,U456            # Open a DM conversation
slackers dm send --users U123 --message "Hi"  # Open and send in one step
```

### Batch Operations

```bash
slackers batch send --message "msg" --channels C1,C2    # Multi-channel send
slackers batch react --emoji rocket --messages url1,url2 # Multi-message react
```

### Workflows & Slash Commands

```bash
slackers workflow list <channel>              # List channel workflows
slackers workflow preview <trigger-id>        # Preview workflow metadata
slackers workflow get <id>                    # Get workflow definition
slackers workflow run <trigger-id> --channel <channel>  # Run a workflow
slackers slash run --channel <channel> /remind me "standup" every weekday at 9am
```

### Saved Items, Scheduled Messages, Mentions, Export

```bash
slackers later add --channel C... --ts <ts>   # Save for later
slackers later list                           # List saved items
slackers later remove --channel C... --ts <ts>

slackers scheduled send --channel C... --message "Hi" --at "2026-12-31T09:00:00Z"
slackers scheduled list
slackers scheduled delete --channel C... --id Q123

slackers mention list --after 2026-01-01      # Your @mentions
slackers mention list --username alice        # Someone else's

slackers export channel --channel "#general" --format csv --output general.csv
```

## Output Format

All commands output compact JSON by default. Use `--pretty` for indented JSON.

Many list/tabular commands support `--format`:

| Value | Description |
|-------|-------------|
| `json` | Pretty-printed JSON (default) |
| `table` | ASCII table |
| `markdown` | GitHub-flavored Markdown table |
| `plain` | Tab-separated or key=value lines |

## Error Handling

Errors are returned as structured JSON with typed exit codes:

| Exit Code | Meaning |
|-----------|---------|
| 0 | Success |
| 1 | General error |
| 3 | Authentication error |
| 4 | Network error |
| 5 | Slack API error |

Error response format:
```json
{
  "error": "auth_failed",
  "type": "auth",
  "message": "Token has been revoked",
  "retryable": false
}
```

## Development

### Building

```bash
cargo build                  # Debug build
cargo build --release        # Release build
```

### Testing

```bash
cargo test                   # Run all tests
cargo test auth::            # Run auth tests
cargo test slack::search::   # Run search tests
```

### Project Structure

- `src/auth/` - Authentication and credential management
- `src/commands/` - CLI command handlers
- `src/mcp/` - MCP server (protocol, tools, stdio transport)
- `src/slack/` - Slack API client and operations
- `src/render/` - Slack format to Markdown conversion (Block Kit, mrkdwn, HTML)
- `src/util/` - Utility functions (LevelDB, redaction)

## License

MIT License - see LICENSE file for details

## Contributing

Contributions are welcome. Please ensure:
- All tests pass (`cargo test`)
- Code follows Rust formatting (`cargo fmt`)
- New features include tests
- Public APIs are documented

## Acknowledgments

Inspired by [agent-slack](https://github.com/stablyai/agent-slack)
