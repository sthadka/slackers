# Output + downloads (reference)

## Output format

All commands print JSON to stdout by default.

- Empty values are pruned (`null`, `[]`, `{}` are removed where possible).
- `auth whoami` redacts secrets in its output.

Pass `--format <fmt>` to list/tabular commands to change the output format:

| Value      | Description                                  |
| ---------- | -------------------------------------------- |
| `json`     | Pretty-printed JSON (default)                |
| `table`    | ASCII table (comfy-table)                    |
| `markdown` | GitHub-flavoured Markdown table              |
| `plain`    | `key=value` or tab-separated lines           |

Aliases: `md` → `markdown`, `text` → `plain`.

Example:

```bash
slackers message list <url> --format table
slackers channel list --format markdown
slackers search messages "deploy" --format plain
```

## Message shapes (high-level)

- `message get` returns:
  - `message: { ... }`
  - `thread?: { ts, length }` (summary only; present when threaded)

- `message list` returns:
  - `messages: [ ... ]` (the full thread)
  - Messages are compact and omit redundant fields on each item where possible.

Use `--max-body-chars` to cap message bodies for token budget control.

## Search shapes (high-level)

- `search messages|all` returns `messages: [ ... ]`
- `search files|all` returns `files: [ ... ]`

Use `--max-content-chars` (messages) and `--limit` to control size.

## Attachment downloads

Attachments are downloaded to an agent-friendly temp directory and returned as absolute paths in output.

Default download root:

- `~/.config/slackers/tmp/downloads/` (Linux)
- `~/Library/Application Support/slackers/tmp/downloads/` (macOS)

If `XDG_RUNTIME_DIR` is set, downloads live under:

- `$XDG_RUNTIME_DIR/slackers/tmp/downloads/`
