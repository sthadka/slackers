# Slackers CLI UX Proposal

## Executive Summary

Slackers is already well-structured — it uses noun-verb naming, singular nouns, flexible identifiers (URLs, #names, IDs), and compact JSON by default. The foundation is solid. The issues are: **one badly misleading command name** (`message list` doesn't list messages), **a few non-standard verbs**, **missing agentic features** from the ideas doc, and **help text that could be more self-documenting**.

---

## 1. Command Renaming

### Critical Renames (confusing / breaks discoverability)

| Current | Proposed | Why |
|---|---|---|
| `message list` | **`message thread`** | This fetches **thread replies**, not a list of messages. An agent or user wanting "list messages in #general" would type `message list #general` and get the wrong thing. `thread` matches what it does and how Slack users think ("show me the thread"). |
| `message history` | **`message list`** | This enumerates channel messages with `--limit`, `--after`, `--before` — that's what `list` means in every CLI (gh, kubectl, docker). Renaming this frees `list` for its conventional meaning. |

This is the single biggest discoverability fix. Today the mapping is backwards:

```
# Current (confusing):
slackers message list #general     → tries to fetch thread replies (?!)
slackers message history #general  → lists channel messages

# Proposed (intuitive):
slackers message thread <url>      → fetch thread replies
slackers message list #general     → list channel messages
```

Slack API alignment: `conversations.history` → `message list`, `conversations.replies` → `message thread`.

### Standard Verb Renames

| Current | Proposed | Why |
|---|---|---|
| `channel new` | **`channel create`** | `create` is the universal CLI verb (gh, kubectl, docker, discli). `new` is a Rails-ism. |
| `message thread-participants` | **`message participants`** | Shorter. It already takes a thread target, so the `thread-` prefix is redundant. |

### Wording Fixes (descriptions, not command names)

| Current Description | Proposed | Why |
|---|---|---|
| `workflow run`: "Trip a workflow trigger" | "Execute a workflow trigger" | "Trip" is jargon. "Execute" or "Run" is standard. |
| `message` (top-level): "Read/write Slack messages (token-efficient JSON)" | "Read, send, and manage Slack messages" | "(token-efficient JSON)" is marketing copy, not a description. |
| `later` (top-level): "Save messages for later (stars) and list saved items" | "Manage saved items (Slack's Later list)" | Cleaner. The parenthetical "(stars)" is legacy terminology. |

### Keep As-Is (already good)

These match industry conventions and Slack's naming:

| Command | Notes |
|---|---|
| `message get`, `message send`, `message delete` | Standard verbs |
| `message react add/remove` | Good nesting |
| `message pin/unpin` | Mirrors Slack |
| `search all/messages/files` | Mirrors Slack API (`search.messages`, etc.) |
| `channel list/get/join/leave/mark/members/invite/rename` | All standard |
| `user list/get` | Standard |
| `file upload/delete/list` | Standard |
| `dm open/send` | Useful convenience shortcut |
| `scheduled send/list/delete` | Maps to Slack concept |
| `batch send/react` | Clear purpose |
| `later add/remove/list` | `later` matches Slack's current UI sidebar label |
| `canvas get` | Fine |
| `serve` | Standard for MCP/daemon mode |

### On `later` — Keep It

Initially considered renaming `later` to `star` or `saved`, but `later` actually matches **Slack's current UI terminology** — the sidebar section is called "Later" and the action is "Save for later." The API still calls them `stars.*` but the UI has moved on. Keep `later` but improve the help text to mention synonyms ("stars", "saved items") for discoverability.

### On `message update` vs `message edit`

gh uses `edit`, Slack's UI says "Edit message." But `update` matches the Slack API method (`chat.update`). Either is fine. Keeping `update` since it matches the API, and agents will find it via help text regardless.

---

## 2. Channel Name Resolution

### The Problem

Passing `#channel-name` to any command triggers `find_channel_by_name()` in `src/slack/channels.rs`, which paginates through `conversations.list` (200 per page) until it finds a match. On large workspaces (2,500+ channels), this means 13+ API calls at Tier 2 rate limits (~20 req/min), causing significant delays and rate limiting.

### API Research: No Direct Lookup Exists

**There is no Slack API method that accepts a channel name and returns its ID** for standard (Free/Pro/Business+) workspaces. This is a widely-known gap with open feature requests across every official Slack SDK.

- `conversations.info` — requires channel ID, does not accept names
- `admin.conversations.search` — has a `query` parameter for name search, but is **Enterprise Grid only**
- No undocumented/internal endpoint is known for channel name lookup

Every Slack CLI tool in the ecosystem uses `conversations.list` + local caching to solve this.

### Current State in Slackers

Slackers already has a cache at `~/.config/slackers/channel-cache.json` (`src/app_config.rs:78`). The `resolve_channel_id()` function (`src/slack/channels.rs:266`) checks the cache first, and only paginates `conversations.list` on a cache miss. Individual lookups are cached after resolution.

**The problem**: on a cold cache or a cache miss, it does a full paginated scan but only caches the one channel it found — it doesn't populate the cache with all channels it saw during the scan. So the next lookup of a *different* channel name repeats the full scan.

### Recommendations (in priority order)

**R1. Bulk-populate the cache during scans (quick fix, high impact)**

When `find_channel_by_name()` paginates through `conversations.list`, cache ALL channel name→ID mappings it encounters, not just the one it was looking for. This means the first lookup is slow, but all subsequent lookups are instant.

```rust
// In find_channel_by_name(), while iterating channels:
for channel in channels {
    if let (Some(name), Some(id)) = (channel["name"].as_str(), channel["id"].as_str()) {
        cache.insert(name.to_string(), id.to_string());
        if name == target_name {
            found_id = Some(id.to_string());
        }
    }
}
// Save full cache, return found_id
```

**R2. Add `limit=999` to reduce pagination (quick fix)**

Currently using `limit=200`. Slack allows up to 999 per page. A 2,500-channel workspace goes from 13 API calls to 3.

**R3. Add a `channel cache refresh` command (low effort)**

Let users proactively populate the cache:

```bash
slackers channel cache          # Build/refresh the channel name cache
slackers channel cache --clear  # Clear the cache
```

This is what slack-user-cli does with its `refresh` command.

**R4. Add cache TTL with stale-while-revalidate (medium effort)**

Store a timestamp with the cache. After 24h, serve the stale cache immediately but trigger a background refresh. This is the pattern korotovsky/slack-mcp-server uses for Enterprise Grid workspaces.

**R5. Recommend channel IDs in help text and AGENTS.md**

For agents specifically, channel IDs are always preferable — they skip resolution entirely:

```markdown
# In AGENTS.md:
- Prefer channel IDs (C...) over #names to avoid API calls
- Use `slackers channel list` to get IDs, then use those IDs in subsequent commands
- Channel name resolution requires API calls and may be slow on large workspaces
```

**R6. Consider `exclude_archived=true` (already done?) and `types` filtering**

Ensure the resolution scan excludes archived channels and only searches the types that matter (public + private, skip mpim/im) to minimize results per page.

### Impact

R1 + R2 together would fix the worst of the problem: first lookup goes from ~13 API calls to ~3, and every subsequent lookup is instant from cache. R5 eliminates the problem entirely for agents.

---

## 3. Help Text Improvements

### Problem: Descriptions are too vague for agents

An agent reading `--help` needs to learn the tool in one shot. Current help text is good but misses three things agents rely on:

**A. Inline allowed values for flags**

```
# Current:
--format <string>    Output format (default: json)
--sort <string>      Sort order

# Proposed:
--format json|table|markdown|plain    Output format [default: json]
--sort timestamp|relevance            Sort order [default: timestamp]
```

This eliminates trial-and-error. Agents won't hallucinate `--format csv` or `--sort date`.

**B. Examples section per command**

Every subcommand should have 2-3 examples in its `--help` output. gh does this and it's the single most-read section. Clap supports this with `#[command(after_help = "...")]` or `#[command(after_long_help = "...")]`.

```
Examples:
  # List recent messages in #general
  slackers message list #general --limit 20

  # List with date filter, compact JSONL
  slackers message list #general --after 2026-08-01 --format jsonl

  # Agent: just IDs and text, piped
  slackers message list #general --limit 5 --format jsonl | head -3
```

**C. Type/format hints for IDs**

The current help is already decent here (`Channel ID (C...) or #name/name`), but it should be consistent everywhere. Some flags say just "Channel ID" without the format hint.

### Proposed top-level help structure

Following the gh pattern:

```
slackers - Slack CLI for humans and AI agents

USAGE
  slackers <command> <subcommand> [flags]

CORE COMMANDS
  message     Read, send, and manage messages and threads
  channel     Discover and manage channels
  search      Search messages and files
  dm          Send direct messages
  unreads     Show unread messages

CONTENT
  file        Upload, delete, and list files
  canvas      Read Slack canvases
  export      Export channel history

WORKSPACE
  user        Look up users
  emoji       List custom emoji
  workspace   Workspace info (name, domain, icon)

AUTOMATION
  scheduled   Schedule and manage delayed messages
  workflow    Discover and run Slack workflows
  batch       Send messages or react in bulk
  slash       Execute slash commands

OTHER
  later       Manage saved items (Later list)
  mention     List @mentions
  auth        Manage authentication
  serve       Start MCP server over stdio

GLOBAL FLAGS
  --read-only     Block all write operations
  --pretty        Indented JSON output
  --quiet         Minimal JSON for write operations
  --no-progress   Suppress stderr spinners

Use 'slackers <command> --help' for more information about a command.
```

Key changes from current:
- **Grouped by domain** instead of flat alphabetical — humans and agents both navigate faster
- **Short descriptions** (5-8 words) — enough to pick the right group
- **Footer shows drill-down hint** — teaches the `<command> --help` pattern

---

## 4. Agentic CLI Features

Prioritized by impact, mapped to what slackers already has vs. what's missing:

### Already Done (keep/polish)

| Feature | Status |
|---|---|
| Compact JSON by default | `--pretty` to opt into indented |
| `--quiet` mode | Suppresses non-essential output |
| `--no-progress` | Kills spinners on stderr |
| `--format jsonl` | NDJSON streaming |
| `--read-only` | Safety guard for agents |
| Structured exit codes | 0/1/3/4/5 already defined |
| `serve` (MCP) | 47 tools over stdio |
| Flexible identifiers | URLs, #names, IDs all accepted |
| stdout/stderr discipline | Progress on stderr, data on stdout |

### Priority 1: Add `--json <fields>` (High Impact, Moderate Effort)

The single most impactful feature from the ideas doc. Currently slackers dumps all fields. With field selection:

```bash
# Current: dumps everything
slackers message list #general --limit 5

# Proposed: select only what you need
slackers message list #general --limit 5 --json ts,user,text

# Self-documenting: no value lists available fields
slackers message list --json
# Available fields: ts, user, text, thread_ts, reply_count, reactions, ...
```

Token savings: 4-10x for typical list operations. This is the gh pattern and it's the gold standard for a reason.

### Priority 2: Add AGENTS.md (High Impact, Low Effort)

Ship a file that agents read before invoking the CLI:

```markdown
# AGENTS.md
## Quick Start
- Use `--format jsonl` for streaming, `--quiet` for write ops
- All write ops are blocked when `--read-only` is set
- Target messages by Slack URL (preferred) or --channel + --ts
- Never parse table output — use `--format json` or `jsonl`

## Common Patterns
# Check unreads
slackers unreads show --counts-only

# Read a thread
slackers message thread <slack-url> --limit 50

# Send a reply
slackers message send <slack-url> "response text"

# Search recent messages
slackers search messages "query" --after 2026-08-01 --limit 10
```

### Priority 3: Structured Error JSON (Medium Impact, Medium Effort)

Current errors are text on stderr. In `--format json` mode, errors should also be JSON:

```json
{"error": "channel not found", "code": "CHANNEL_NOT_FOUND", "input": "#nonexistent", "hint": "use 'slackers channel list' to find valid channels"}
```

Error codes agents can match on: `AUTH_FAILED`, `CHANNEL_NOT_FOUND`, `RATE_LIMITED`, `PERMISSION_DENIED`, `INVALID_INPUT`.

### Priority 4: Pipe/TTY Detection (Medium Impact, Low Effort)

Auto-detect when stdout is piped and switch behavior:
- TTY: table format (human-friendly), colors, progress
- Piped: JSON format (machine-friendly), no color, no progress

```bash
# TTY (human watching)
slackers channel list
# Shows pretty table

# Piped (agent/script consuming)
slackers channel list | jq '.[] .name'
# Outputs JSON automatically
```

### Priority 5: `describe` Command (Lower Impact, Low Effort)

Schema introspection for agents, following the Algolia pattern:

```bash
slackers describe message list
```
```json
{
  "command": "message list",
  "args": [{"name": "channel", "type": "string", "required": true, "format": "C... or #name"}],
  "flags": [
    {"name": "--limit", "type": "integer", "default": 500},
    {"name": "--format", "type": "string", "values": ["json","table","markdown","plain"], "default": "json"}
  ],
  "output_fields": ["ts", "user", "text", "thread_ts", "reply_count", "reactions"]
}
```

This costs tokens once (on discovery) but prevents many failed attempts.

---

## 5. Structural Simplifications

### Default subcommands for single-action resources

Five commands have only one subcommand. Add a default action so the bare command works:

| Current (requires subcommand) | Proposed (default action) |
|---|---|
| `slackers workspace info` | `slackers workspace` also works (defaults to `info`) |
| `slackers emoji list` | `slackers emoji` also works (defaults to `list`) |
| `slackers mention list` | `slackers mention` also works (defaults to `list`) |
| `slackers unreads show` | `slackers unreads` also works (defaults to `show`) |
| `slackers slash run` | `slackers slash` also works (defaults to `run`) |

The subcommand form still works too — this just removes unnecessary friction. In clap, use `#[command(subcommand_required = false)]` with a default handler.

This also future-proofs: when you add `emoji search` or `workspace billing`, the subcommand pattern is already there.

---

## 6. MCP Tool Name Alignment

The MCP tools in `tools.rs` use underscore names that map 1:1 to CLI commands. After renaming:

| Current MCP Tool | Proposed | CLI Equivalent |
|---|---|---|
| `message_list` | `message_thread` | `message thread` |
| `message_history` | `message_list` | `message list` |
| `message_thread_participants` | `message_participants` | `message participants` |
| `channel_new` | `channel_create` | `channel create` |

All other tool names stay the same.

---

## 7. Summary of All Changes

### Must-Do (high confusion/misalignment today)
1. Rename `message list` → `message thread`
2. Rename `message history` → `message list`
3. Rename `channel new` → `channel create`
4. Group top-level help by domain (not flat alphabetical)
5. Add inline allowed values to all enum flags

### Should-Do (significant improvement)
6. Add `--json <fields>` field selection
7. Ship AGENTS.md
8. Add examples to every subcommand's `--help`
9. Rename `message thread-participants` → `message participants`
10. Add default actions for single-subcommand resources

### Nice-to-Have (polish)
11. Structured error JSON with error codes
12. Pipe/TTY auto-detection
13. `describe` command for schema introspection
14. Fix description wording ("Trip" → "Execute", remove "token-efficient JSON")

### Explicitly Not Changing
- `later` — matches Slack's current UI terminology
- `message get` — `get` is fine, no need to switch to `view`
- `message update` — matches Slack API, no need to switch to `edit`
- `search all/messages/files` — already mirrors Slack API
- `dm open/send` — useful convenience, not duplicative
- `serve` — standard for MCP server mode
