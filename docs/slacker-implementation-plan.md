# Plan: `slackers` — Rust Clone of `agent-slack`

## Context

The TypeScript CLI tool `agent-slack` (~30 source files) provides a Slack automation CLI designed for AI agents. It supports multi-workspace auth (standard tokens + browser xoxc/xoxd), reading/writing messages, search, canvas fetching, user lookups, and macOS-specific credential extraction (Desktop LevelDB, Chrome AppleScript, Keychain). All output is JSON, pruned of null/empty fields.

We are building a full-parity Rust clone called `slackers` in `~/code/experiments/slackers/` (currently empty). Config will be stored at `~/.config/slackers/`.

Reference source: `~/code/experiments/agent-slack/src/`

---

## Project Structure

```
slacke-rs/
├── Cargo.toml
└── src/
    ├── main.rs                     # Entry point, clap dispatch
    ├── cli.rs                      # All clap derive structs
    ├── error.rs                    # thiserror error types
    ├── output.rs                   # JSON pruning + pretty-print
    ├── config.rs                   # Paths, credential file I/O
    ├── target.rs                   # URL/channel/ID target parsing
    ├── auth/
    │   ├── mod.rs
    │   ├── types.rs                # Credential structs (serde)
    │   ├── resolver.rs             # Env → config → desktop → chrome fallback chain
    │   ├── store.rs                # Load/save/upsert/remove credentials
    │   ├── keychain.rs             # macOS Keychain via `security` CLI
    │   ├── desktop.rs              # Slack Desktop LevelDB + SQLite extraction
    │   ├── chrome.rs               # Chrome AppleScript extraction
    │   ├── curl.rs                 # Parse cURL command for xoxc/xoxd
    │   └── commands.rs             # auth subcommand handlers
    ├── slack/
    │   ├── mod.rs
    │   ├── client.rs               # HTTP client (standard + browser modes, retry)
    │   ├── channels.rs             # Channel name → ID resolution
    │   ├── messages.rs             # Fetch message/thread, CompactSlackMessage
    │   ├── users.rs                # List/get users, handle → ID resolution
    │   ├── search.rs               # Search orchestration (API vs channel fallback)
    │   ├── search_query.rs         # Build search query string, date utils
    │   ├── search_raw.rs           # Raw search.messages / search.files API
    │   ├── search_messages.rs      # Search messages via API + channel fallback
    │   ├── search_files.rs         # Search files via API + channel fallback
    │   ├── files.rs                # Authenticated file download + caching
    │   ├── canvas.rs               # Canvas URL parsing + fetch → HTML → Markdown
    │   └── emoji.rs                # Shortcode normalization + Unicode lookup
    ├── render/
    │   ├── mod.rs
    │   ├── blocks.rs               # rich_text/section/actions/context/image blocks → mrkdwn
    │   ├── mrkdwn.rs               # Slack mrkdwn → standard Markdown
    │   ├── attachments.rs          # Legacy attachment rendering
    │   └── html_to_md.rs           # HTML → Markdown (for canvases)
    ├── commands/
    │   ├── mod.rs                  # Top-level dispatch
    │   ├── message.rs              # message get/list/send/react handlers
    │   ├── search.rs               # search all/messages/files handlers
    │   ├── canvas.rs               # canvas get handler
    │   └── user.rs                 # user list/get handlers
    └── util/
        ├── mod.rs
        ├── redact.rs               # Token redaction for display
        └── leveldb.rs              # Pure-Rust LevelDB key scanner
```

---

## Dependencies (`Cargo.toml`)

```toml
[package]
name = "slackers"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "slackers"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
regex = "1"
url = "2"
dirs = "6"
chrono = { version = "0.4", features = ["serde"] }
emojis = "0.6"
rusqlite = { version = "0.32", features = ["bundled"] }  # For Slack Desktop cookie DB
html2md = "0.2"                                          # HTML → Markdown for canvases
snap = "1"                                                # Snappy decompression for LevelDB
urlencoding = "2"                                         # URL encode/decode
```

---

## Implementation Phases

### Phase 1: Skeleton + CLI Parsing
**Files:** `Cargo.toml`, `src/main.rs`, `src/cli.rs`, `src/error.rs`, `src/commands/mod.rs`

- Initialize Cargo project with all dependencies
- Define all clap structs matching the full command tree:
  - `auth` → `whoami | test | add | set-default | remove | import-desktop | import-chrome | parse-curl`
  - `message` → `get | list | send | react {add|remove}`
  - `search` → `all | messages | files` (shared `SearchArgs`)
  - `canvas` → `get`
  - `user` → `list | get`
- Dispatch function in `commands/mod.rs` that matches on command enum
- Error types in `error.rs`
- **Verify:** `cargo build` succeeds, `slackers --help` shows all commands

### Phase 2: Config + Auth Types + Store
**Files:** `src/config.rs`, `src/auth/types.rs`, `src/auth/store.rs`, `src/auth/keychain.rs`

**Config paths** (in `config.rs`):
- `~/.config/slackers/credentials.json`
- `~/.slackers/tmp/downloads/`
- `~/.config/slackers/cache/leveldb-snapshots/`

**Credential types** (mirror `auth/schema.ts`):
```rust
#[derive(Serialize, Deserialize)]
struct Credentials { version: u32, updated_at: Option<String>, default_workspace_url: Option<String>, workspaces: Vec<Workspace> }

#[derive(Serialize, Deserialize)]
struct Workspace { workspace_url: String, workspace_name: Option<String>, team_id: Option<String>, team_domain: Option<String>, auth: WorkspaceAuth }

#[derive(Serialize, Deserialize)]
#[serde(tag = "auth_type")]
enum WorkspaceAuth {
    #[serde(rename = "standard")] Standard { token: String },
    #[serde(rename = "browser")] Browser { xoxc_token: String, xoxd_cookie: String },
}
```

**Store** (mirror `auth/store.ts`): `load_credentials`, `save_credentials`, `upsert_workspace`, `upsert_workspaces`, `set_default_workspace`, `remove_workspace`, `resolve_workspace_for_url`, `resolve_default_workspace`. All normalize workspace URLs to `{protocol}//{host}`.

**Keychain** (mirror `auth/keychain.ts`): Shell out to macOS `security` CLI. `keychain_get(account, service)`, `keychain_set(account, value, service)`. On save, store tokens in Keychain and write `__KEYCHAIN__` placeholder to file. On load, hydrate from Keychain.

### Phase 3: Auth Resolver + Slack API Client
**Files:** `src/auth/resolver.rs`, `src/slack/client.rs`, `src/output.rs`

**Resolver** (mirror `cli/context.ts` getClientForWorkspace):
Priority chain:
1. `SLACK_TOKEN` env var (+ `SLACK_COOKIE_D`/`SLACK_COOKIE` if xoxc)
2. Stored credential by workspace URL
3. Default workspace from credentials
4. Auto-extract from Slack Desktop (macOS)
5. Fallback: Chrome extraction (macOS)
6. Error with helpful message

Returns `ResolvedAuth { auth: WorkspaceAuth, workspace_url: Option<String> }`.

**SlackClient** (mirror `slack/client.ts`):
```rust
struct SlackClient { http: reqwest::Client, auth: WorkspaceAuth, workspace_url: Option<String> }
```
- `api_call(&self, method, params) -> Result<serde_json::Value>`
- Standard token: POST `https://slack.com/api/{method}`, `Authorization: Bearer {token}`, form body
- Browser token: POST `{workspace_url}/api/{method}`, `Cookie: d={url_encode(xoxd)}`, `Content-Type: application/x-www-form-urlencoded`, `Origin: https://app.slack.com`, body includes `token={xoxc}` + params
- Retry on 429: parse `Retry-After` header, sleep min(header_val, 30)s, max 3 retries
- Check `ok: true` in response, return error with `error` field on failure

**Output** (mirror `lib/compact-json.ts`):
- `prune_empty(Value) -> Value`: recursively remove null, empty strings, empty arrays, empty objects
- `to_json_output<T: Serialize>(value) -> String`: serialize → prune → pretty-print (2-space)

### Phase 4: Auth Commands
**Files:** `src/auth/commands.rs`, `src/util/redact.rs`

Implement all auth subcommands:
- **whoami**: Load credentials, redact tokens, output JSON
- **test**: Resolve auth, call `auth.test`, output response
- **add**: Validate `--token` or `--xoxc`+`--xoxd`, upsert, print confirmation
- **set-default**: Set default workspace URL
- **remove**: Remove workspace, clear default if removed
- **import-desktop**: Call desktop extraction, upsert workspaces (Phase 6)
- **import-chrome**: Call chrome extraction, upsert workspaces (Phase 6)
- **parse-curl**: Read stdin, parse cURL, upsert workspace (Phase 6)

`redact_secret(value, keep_start=6, keep_end=4)` — same logic as TS.

### Phase 5: Target Parsing + Channel Resolution
**Files:** `src/target.rs`, `src/slack/channels.rs`

**Target parsing** (mirror `cli/targets.ts` + `slack/url.ts`):
```rust
enum MsgTarget {
    Url(SlackMessageRef),
    Channel(String),
}
struct SlackMessageRef { workspace_url: String, channel_id: String, message_ts: String, thread_ts_hint: Option<String>, raw: String, possibly_truncated: bool }
```
- URL pattern: `https://*.slack.com/archives/{channel_id}/p{digits}?thread_ts=...`
- `p{digits}` → split at len-6: `{seconds}.{micros}`
- Truncation detection: has `thread_ts` param but no `cid` param

**Channel resolution** (mirror `slack/channels.ts`):
- `is_channel_id(input)`: matches `^[CDG][A-Z0-9]{8,}$`
- `normalize_channel_input(input)` → `{kind: id|name, value}`
- `resolve_channel_id(client, input)`: paginate `conversations.list` (types=public_channel,private_channel), match by name

### Phase 6: macOS Auth Extraction
**Files:** `src/auth/desktop.rs`, `src/auth/chrome.rs`, `src/auth/curl.rs`, `src/util/leveldb.rs`

**Desktop extraction** (mirror `auth/desktop.ts`):
- Find Slack data: `~/Library/Application Support/Slack/` or MAS container
- Snapshot LevelDB dir (copy-on-write clone via `cp -cR`)
- Scan LevelDB files for `localConfig_v2`/`localConfig_v3` keys using pure-Rust reader
  - Parse `.ldb`/`.sst` files (table format) and `.log` files
  - Use `snap` crate for Snappy decompression of data blocks
- Parse config value: handle leading byte prefix, try UTF-8/UTF-16LE, extract JSON
- Extract team objects: `{ url, name, token }` where token starts with `xoxc-`
- Extract `d` cookie from SQLite `Cookies` DB via `rusqlite`
  - Query: `SELECT encrypted_value FROM cookies WHERE name='d' AND host_key LIKE '%slack.com'`
  - Decrypt using macOS Safe Storage password (PBKDF2 + AES-128-CBC, same as Chromium)
  - Safe Storage password from Keychain: `security find-generic-password -w -s "Slack Safe Storage"`

**Chrome extraction** (mirror `auth/chrome.ts`):
- macOS only, uses `osascript` to query Chrome tabs
- Cookie script: find Slack tab, execute JS to extract `document.cookie` `d=` value
- Teams script: execute JS to read `localStorage.localConfig_v2/v3`, extract team objects
- Parse JSON, filter for `xoxc-` tokens

**cURL parsing** (mirror `auth/curl.ts`):
- Regex to extract workspace URL, `d=xoxd-...` cookie, `xoxc-...` token from cURL command text
- Read from stdin

### Phase 7: Content Rendering
**Files:** `src/render/blocks.rs`, `src/render/mrkdwn.rs`, `src/render/attachments.rs`, `src/slack/emoji.rs`

**Block rendering** (mirror `slack/render.ts`):
- `render_message_content(msg: &Value) -> String`: try blocks → attachments → text fallback
- Block types: `section` (text + fields + accessory button), `rich_text` (recursive), `actions` (buttons), `context` (elements), `image` (URL)
- Rich text elements: `rich_text_section` (inline), `rich_text_preformatted` (code block), `rich_text_quote` (> prefix), `rich_text_list` (ordered/bullet)
- Inline elements: `text` (with bold/italic/strike/code styles), `link`, `emoji`, `user`, `channel`
- Output is Slack mrkdwn format (then converted to Markdown)

**Mrkdwn conversion** (mirror `slack/mrkdwn.ts`):
- Regex replacements in order:
  1. `<URL|label>` → `[label](URL)`
  2. `<URL>` → `URL`
  3. `<#C123|name>` → `#name`
  4. `<@U123|name>` → `@name`, `<@U123>` → `@U123`
  5. `<!here>` → `@here` etc.
  6. HTML entities: `&lt;` `&gt;` `&amp;`
  7. Emoji shortcodes → Unicode via `emojis` crate

**Attachment rendering** (mirror `slack/render.ts` extractMrkdwnFromAttachments):
- Process: blocks in attachment, pretext, title/title_link, text, fields, fallback

**Emoji** (mirror `slack/emoji.ts`):
- `normalize_reaction_name(input)`: accept `:rocket:`, `rocket`, or Unicode `🚀` → `rocket`
- Use `emojis` crate for Unicode ↔ shortcode conversion

### Phase 8: Message Commands
**Files:** `src/commands/message.rs`, `src/slack/messages.rs`, `src/slack/files.rs`

**Messages module** (mirror `slack/messages.ts`):
- `fetch_message(client, ref)`: `conversations.history` with `latest=ts, inclusive=true, limit=5`, find exact ts match. Fallback: scan thread via `conversations.replies`. Fallback: try as thread root.
- `fetch_thread(client, channel_id, thread_ts)`: paginate `conversations.replies`, sort chronologically
- `to_compact_message(msg, options)`: render content, truncate, build `CompactSlackMessage`
- File enrichment: call `files.info` for snippets or files missing download URL

**File download** (mirror `slack/files.ts`):
- Download to `~/.slackers/tmp/downloads/`
- Auth headers: standard → `Authorization: Bearer {token}`, browser → `Bearer {xoxc}` + `Cookie: d={xoxd}` + `Referer`
- Skip if cached (file exists). Sanitize filename. Reject HTML responses (auth failure).

**Command handlers** (mirror `cli/message-actions.ts`):
- `message get`: parse target, resolve auth+channel, fetch message, get thread summary, download files, output JSON
- `message list`: parse target, resolve, fetch full thread, download files, output JSON (strip channel_id/thread_ts per message)
- `message send`: parse target, resolve, `chat.postMessage` (auto-thread if URL target), output `{ ok: true }`
- `message react add/remove`: parse target, resolve, normalize emoji, call `reactions.add/remove`, output `{ ok: true }`

Auto-refresh on auth error: if not env auth and error is `invalid_auth`/`token_expired`, try desktop refresh and retry.

### Phase 9: Search Commands
**Files:** `src/commands/search.rs`, `src/slack/search.rs`, `src/slack/search_query.rs`, `src/slack/search_raw.rs`, `src/slack/search_messages.rs`, `src/slack/search_files.rs`

**Search query building** (mirror `slack/search-query.ts`):
- Append `after:`, `before:`, `from:@user`, `in:#channel` modifiers
- Resolve user IDs to names, channel IDs to names for search syntax

**Two search paths** (mirror `slack/search.ts`):
1. No channel filter: use `search.messages`/`search.files` API (paginated)
2. With channel filter: fall back to `conversations.history`/`files.list` with client-side filtering

**Content type filter**: `any|text|image|snippet|file` applied post-fetch

**Defaults**: limit=20 (max 200), max_content_chars=4000

### Phase 10: Canvas + User Commands
**Files:** `src/commands/canvas.rs`, `src/slack/canvas.rs`, `src/render/html_to_md.rs`, `src/commands/user.rs`, `src/slack/users.rs`

**Canvas** (mirror `slack/canvas.ts`):
- Parse canvas URL: `https://*.slack.com/docs/T.../F...` or bare `F...` ID
- `files.info` → download HTML → convert to Markdown via `html2md` crate
- Extract `<main>`/`<article>`/`<body>` content before conversion
- Truncate to `--max-chars` (default 20000)

**Users** (mirror `slack/users.ts`):
- `list_users`: paginate `users.list`, filter bots unless `--include-bots`, compact output
- `get_user`: resolve by ID (`U...`) or by handle (`@name`/`name`), call `users.info`
- `CompactSlackUser`: `id, name, real_name, display_name, email, title, tz, is_bot, deleted`

---

## Key Design Decisions

1. **Async with tokio** — all Slack API calls are I/O-bound
2. **`serde_json::Value` for API responses** — Slack's response shapes are highly variable; parse into Value, extract fields manually (matching the TS approach of `isRecord`/`getString` guards)
3. **`anyhow` for app errors, `thiserror` for typed errors** — pattern match on auth errors for auto-refresh logic
4. **Config at `~/.config/slackers/`** — separate from the TS tool per user preference
5. **Credential interop** — same JSON schema so credentials could be copied between tools if needed
6. **LevelDB reader** — implement a minimal pure-Rust reader (parse table files + log files, Snappy decompress) rather than depending on a full LevelDB binding. Only need key-value scanning for `localConfig_v*` keys.

---

## Verification

After each phase, verify with:

1. **Phase 1**: `cargo build && ./target/debug/slackers --help` — all commands/subcommands visible
2. **Phase 2**: `cargo test` — unit tests for credential serialization/deserialization roundtrip
3. **Phase 3**: `SLACK_TOKEN=xoxb-test slackers auth test` — verifies client + resolver + output
4. **Phase 4**: `slackers auth whoami`, `slackers auth add --workspace-url https://test.slack.com --token xoxb-...`, `slackers auth remove https://test.slack.com`
5. **Phase 5**: Unit tests for URL parsing (various URL formats, edge cases, truncation detection)
6. **Phase 6**: `slackers auth import-desktop` on macOS with Slack Desktop installed
7. **Phase 7**: Unit tests for mrkdwn→Markdown conversion, block rendering with sample Slack payloads
8. **Phase 8**: `slackers message get "https://team.slack.com/archives/C.../p..."`, `slackers message send "#general" "hello"`
9. **Phase 9**: `slackers search messages "test query" --limit 5`
10. **Phase 10**: `slackers user list --limit 10`, `slackers canvas get F...`

**End-to-end**: With real credentials, run the full command suite against a test Slack workspace and compare JSON output structure with the TypeScript version.
