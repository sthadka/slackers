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
