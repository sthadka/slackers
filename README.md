# slackers

A comprehensive command-line interface for Slack workspace management and API operations, written in Rust.

## Overview

slackers is a full-featured Slack CLI that provides complete workspace management, message operations, search capabilities, and authentication handling. It offers a fast, reliable alternative to web-based Slack interactions with support for multiple workspaces and credential sources.

## Features

### Message Operations
- Fetch individual messages and entire threads
- Send messages to channels and threads
- Add reactions to messages
- List and filter thread messages with advanced criteria
- Download file attachments automatically
- Filter messages by user, links, files, or reactions
- Limit and paginate message results

### Search
- Search messages across workspaces
- Search files with content type filtering
- Unified search (messages and files)
- Advanced query syntax with date ranges, user filters, and channel scoping
- Support for content type filtering (text, image, snippet)

### Canvas Documents
- Fetch Slack canvas documents
- Convert to Markdown format
- Support for both URLs and file IDs
- Configurable content truncation

### User Management
- List workspace users
- Get user details by ID or name
- Filter active users and bots

### Channel Discovery & Management
- List all channels with type filtering (public, private, DMs, multi-party DMs)
- Get detailed channel information (topic, purpose, member count)
- Join and leave channels programmatically
- Support for both channel IDs and names with auto-resolution
- Pagination for workspaces with many channels

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
git clone https://github.com/yourusername/slackers.git
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

Get canvas document:
```bash
slackers canvas get https://yourteam.slack.com/docs/T123/F456
```

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
slackers message react <target> <emoji>       # Add reaction
```

Message list options:
- `--limit` - Maximum messages to return
- `--user` - Filter by user ID or @handle
- `--has-link` - Only messages with links
- `--has-file` - Only messages with file attachments
- `--has-reaction` - Only messages with reactions
- `--after-ts`, `--before-ts` - Time-range filtering

Example:
```bash
# Get first 10 messages from a user with file attachments
slackers message list <thread-url> --user "@alice" --has-file --limit 10
```

### Search

```bash
slackers search all <query>                   # Search messages and files
slackers search messages <query>              # Search messages
slackers search files <query>                 # Search files
```

Search options:
- `--channel` - Filter by channel (repeatable)
- `--user` - Filter by user
- `--after` - Results after date (YYYY-MM-DD)
- `--before` - Results before date (YYYY-MM-DD)
- `--content-type` - Filter by type (any, text, image, snippet, file)
- `--limit` - Maximum results (default 20, max 200)

### Canvas

```bash
slackers canvas get <url-or-id>               # Get canvas as Markdown
```

### Users

```bash
slackers user list                            # List all users
slackers user get <identifier>                # Get user details
```

### Channels

```bash
slackers channel list                         # List all channels
slackers channel get <channel>                # Get channel info
slackers channel join <channel>               # Join a channel
slackers channel leave <channel>              # Leave a channel
```

Channel options:
- `--types` - Filter by type (public_channel, private_channel, mpim, im)
- `--exclude-archived` - Exclude archived channels (default: true)
- `--include-num-members` - Include member count in response
- `--limit` - Maximum channels to return (default: 200)

**Performance tip:** Use channel IDs (e.g., `C0123456789`) instead of names for faster operations. Get IDs from `channel list` first.

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
- `src/slack/` - Slack API client and operations
- `src/render/` - Slack format to Markdown conversion
- `src/util/` - Utility functions (LevelDB, redaction)

## Implementation Notes

### macOS Features

The macOS-specific features use:
- LevelDB reader (rusty-leveldb) for Slack Desktop storage
- SQLite (rusqlite) for cookie databases
- PBKDF2 + AES-128-CBC for encrypted cookie decryption
- macOS Keychain for Safe Storage password retrieval
- AppleScript (osascript) for Chrome tab interaction

### Security

- Credentials stored in user config directory
- Secrets redacted in output (first 6, last 4 characters shown)
- LevelDB snapshots cleaned up after extraction
- Browser tokens require both xoxc and xoxd components

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
