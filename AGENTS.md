# Agent Instructions

## Project Overview

**slackers** provides comprehensive Slack workspace management and API operations via command-line interface and is written in Rust.

**Key Components:**
- Message operations (fetch, send, search, react)
- Search API (messages, files, unified search)
- Canvas document fetching and conversion
- User and channel management
- Authentication import from Slack Desktop and Chrome (macOS)
- Multi-workspace credential management

**Technology Stack:**
- Rust 2021 edition with Tokio async runtime
- rusty-leveldb for LevelDB reading
- rusqlite for SQLite (cookies DB)
- PBKDF2 + AES-128-CBC for credential decryption
- macOS Keychain integration via osascript

## Development Workflow

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

### Building and Testing

```bash
# Build the project
cargo build

# Run all tests (103 tests)
cargo test

# Run specific test module
cargo test auth::
cargo test slack::search

# Build release binary
cargo build --release

# Run the CLI
cargo run -- auth whoami
cargo run -- message get <url>
```

### Code Organization

```
src/
├── auth/          # Authentication & credential management
│   ├── chrome.rs  # Chrome extraction (macOS)
│   ├── curl.rs    # cURL command parsing
│   ├── desktop.rs # Slack Desktop extraction (macOS)
│   ├── keychain.rs # macOS Keychain integration
│   ├── resolver.rs # Auth resolution priority chain
│   └── store.rs   # Credential storage
├── commands/      # CLI command handlers
│   ├── auth.rs    # Auth commands
│   ├── canvas.rs  # Canvas commands
│   ├── message.rs # Message commands
│   ├── search.rs  # Search commands
│   └── user.rs    # User commands
├── slack/         # Slack API client & operations
│   ├── search*.rs # Search implementation
│   ├── messages.rs # Message fetching
│   ├── files.rs   # File downloads
│   └── canvas.rs  # Canvas operations
├── render/        # Slack format rendering
│   ├── blocks.rs  # Block Kit to Markdown
│   └── mrkdwn.rs  # Slack markup to Markdown
└── util/          # Utilities
    ├── leveldb.rs # LevelDB scanner
    └── redact.rs  # Secret redaction
```

### Testing Guidelines

- All new features must include unit tests
- Integration tests use environment variables for credentials
- Test names should be descriptive: `test_<component>_<scenario>`
- Mock external dependencies (Slack API calls) where practical
- Keychain tests clean up after themselves

### Common Tasks

**Adding a new Slack API endpoint:**
1. Add method to `slack/client.rs` if needed
2. Create module in `slack/` for the feature
3. Add tests for the module
4. Wire to CLI in `cli.rs` and `commands/`

**Adding a new command:**
1. Define in `cli.rs` using clap derive
2. Create handler in `commands/`
3. Wire to dispatcher in `commands/mod.rs`
4. Add integration test if applicable

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed using [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) format AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:970c3bf2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bd dolt push
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->

---

## Slackers CLI Quick Start

Slackers is a Slack CLI for AI agents and humans. All output goes to stdout as compact JSON by default; progress and errors go to stderr.

### Recommended Flags

| Flag | Purpose |
|---|---|
| `--format jsonl` | Streaming NDJSON — one object per line, ideal for piping |
| `--quiet` | Minimal JSON output for write operations (e.g. `{"ok":true}`) |
| `--no-progress` | Suppress stderr spinners (set this in non-interactive contexts) |
| `--read-only` | Block all write operations — use this as a safety guard |
| `--pretty` | Indented JSON (only for human debugging, wastes tokens) |

Never parse `--format table` output programmatically. Use `json` or `jsonl`.

### Channel IDs Over Names

Prefer channel IDs (`C0123ABCDEF`) over `#channel-name`. Name resolution requires paginating `conversations.list` and can be slow on large workspaces. Use `channel list` once to get IDs, then use those IDs for all subsequent commands.

```bash
# Get channel IDs (do this once, cache the result)
slackers channel list --format jsonl --limit 999

# Then use IDs directly — no API overhead for name resolution
slackers message list C0123ABCDEF --limit 20
```

### Targeting Messages

Most message commands accept a Slack URL as the target. This is the most reliable identifier:

```bash
slackers message get "https://myteam.slack.com/archives/C0123ABC/p1234567890"
```

When you don't have a URL, use `--channel` + `--ts`:

```bash
slackers message get C0123ABCDEF --ts 1234567890.123456
```

## Common Patterns

### Check Unreads

```bash
# Counts only (fast, minimal tokens)
slackers unreads show --counts-only

# With message content
slackers unreads show --max-messages 5
```

### Read a Thread

```bash
# By Slack URL (preferred)
slackers message thread "https://myteam.slack.com/archives/C0123ABC/p1234567890" --limit 50

# By channel + thread timestamp
slackers message thread C0123ABCDEF --thread-ts 1234567890.123456 --limit 50
```

### List Channel Messages

```bash
# Recent messages
slackers message list C0123ABCDEF --limit 20

# With date filter
slackers message list C0123ABCDEF --after 2026-08-01 --before 2026-08-10 --limit 50

# Include thread replies inline
slackers message list C0123ABCDEF --limit 20 --include-threads
```

### Send a Message

```bash
# Reply in a thread (target is the thread root URL)
slackers message send "https://myteam.slack.com/archives/C0123ABC/p1234567890" "Got it, thanks!"

# Post to a channel
slackers message send C0123ABCDEF "Hello from the CLI"

# DM a user
slackers dm send --users U0123ABCDEF --message "Quick question"
```

### Search

```bash
# Search messages
slackers search messages "deployment failed" --after 2026-08-01 --limit 10

# Search in a specific channel
slackers search messages "bug report" --channel C0123ABCDEF --limit 5

# Search files
slackers search files "architecture diagram" --limit 5
```

### Reactions

```bash
slackers message react add "https://myteam.slack.com/archives/C0123ABC/p1234567890" thumbsup
slackers message react remove "https://myteam.slack.com/archives/C0123ABC/p1234567890" thumbsup
```

## Safety

- Always set `--read-only` when exploring or reading data to prevent accidental writes.
- The `--quiet` flag reduces write-operation output but does not suppress errors.
- Structured exit codes: 0 = success, 1 = general error, 3 = auth failure, 4 = not found, 5 = rate limited.
- All write commands (`send`, `delete`, `update`, `pin`, `react add`, `create`) are blocked when `--read-only` is active.

## Command Reference

| Command | Description |
|---|---|
| `message list <channel>` | List channel messages (with `--limit`, `--after`, `--before`) |
| `message thread <target>` | Fetch full thread replies |
| `message get <target>` | Fetch a single message |
| `message send <target> <text>` | Send or reply to a message |
| `message participants <target>` | List thread participants |
| `message pin` / `unpin` | Pin or unpin a message |
| `message react add` / `remove` | Add or remove emoji reactions |
| `message delete` | Delete a message |
| `message update` | Edit a message |
| `channel list` | List workspace channels |
| `channel get <channel>` | Get channel details |
| `channel create --name <name>` | Create a new channel |
| `channel join` / `leave` | Join or leave a channel |
| `channel mark` | Mark channel as read |
| `channel members` | List channel members |
| `search all` / `messages` / `files` | Search Slack content |
| `unreads show` | Show unread messages |
| `dm open` / `send` | Open or send direct messages |
| `file upload` / `delete` / `list` | Manage files |
| `later add` / `remove` / `list` | Manage saved items |
| `scheduled send` / `list` / `delete` | Manage scheduled messages |
| `canvas get` | Fetch a canvas as Markdown |
| `user list` / `get` | Look up users |
| `mention list` | List @mentions |
| `export channel` | Export channel history |
| `batch send` / `react` | Bulk operations |
| `serve` | Start MCP server over stdio |
