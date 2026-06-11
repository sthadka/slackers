# `slackers` command map (reference)

Run `slackers --help` (or `slackers <command> --help`) for the full option list.

## Auth

- `slackers auth whoami` — show configured workspaces + token sources (secrets redacted)
- `slackers auth test [--workspace <url>]` — verify credentials (`auth.test`)
- `slackers auth import-desktop` — import browser-style creds from Slack Desktop (macOS)
- `slackers auth import-chrome` — import creds from Chrome (macOS)
- `slackers auth parse-curl` — read a copied Slack cURL command from stdin and save creds
- `slackers auth add --workspace-url <url> [--token <xoxb/xoxp> | --xoxc <xoxc> --xoxd <xoxd>]`
- `slackers auth set-default <workspace-url>`
- `slackers auth remove <workspace-url>`

## Channels

- `slackers channel list`
  - List all channels/conversations in the workspace
  - Options:
    - `--workspace <url>` (required when you have multiple workspaces)
    - `--types <type>` repeatable (public_channel, private_channel, mpim, im)
    - `--exclude-archived` (default: true)
    - `--limit <n>` (default `200`)

- `slackers channel get <#channel|channel|C...>`
  - Get detailed information about a specific channel
  - Options:
    - `--workspace <url>` (required when using channel name across multiple workspaces)
    - `--include-num-members` (include member count in response)

- `slackers channel join <#channel|channel|C...>`
  - Join a channel
  - Options:
    - `--workspace <url>` (required when using channel name across multiple workspaces)

- `slackers channel leave <#channel|channel|C...>`
  - Leave a channel
  - Options:
    - `--workspace <url>` (required when using channel name across multiple workspaces)

## Messages / threads

- `slackers message get <target>`
  - `<target>`: Slack message URL OR `#channel`/`channel`/channel id (`C...`) (see `targets.md`)
  - Options:
    - `--workspace <url>` (required when using a channel _name_ across multiple workspaces)
    - `--ts <seconds>.<micros>` (required when targeting a channel)
    - `--thread-ts <seconds>.<micros>` (optional hint for thread permalinks)
    - `--max-body-chars <n>` (default `8000`, `-1` unlimited)
    - `--include-reactions`

- `slackers message list <target>`
  - Fetches the full thread with filtering and pagination
  - Options:
    - `--workspace <url>` (same rules as above)
    - `--thread-ts <seconds>.<micros>` (required for channel targets unless you pass `--ts`)
    - `--ts <seconds>.<micros>` (optional: resolve a message to its thread)
    - `--max-body-chars <n>` (default `8000`, `-1` unlimited)
    - `--include-reactions`
    - `--limit <n>` (maximum messages to return, default: 100)
    - `--after-ts <timestamp>` (only messages after this timestamp)
    - `--before-ts <timestamp>` (only messages before this timestamp)
    - `--user <U...|@handle>` (filter by user)
    - `--has-link` (only messages with links)
    - `--has-file` (only messages with file attachments)
    - `--has-reaction` (only messages with reactions)

- `slackers message send <target> <text>`
  - If `<target>` is a Slack message URL, replies in that message's thread.
  - Otherwise posts to the channel/DM.
  - Options:
    - `--workspace <url>` (needed for channel _names_ across multiple workspaces)
    - `--thread-ts <seconds>.<micros>` (optional, channel mode only)

- `slackers message react <target> <emoji>`
  - Options (channel mode):
    - `--workspace <url>` (needed for channel _names_ across multiple workspaces)
    - `--ts <seconds>.<micros>` (required for channel targets)

## Search

- `slackers search all <query>` — messages + files (default)
- `slackers search messages <query>`
- `slackers search files <query>`

Common options:

- `--workspace <url>` (recommended when using channel names across multiple workspaces)
- `--channel <channel...>` repeatable (`#name`, `name`, or id)
- `--user <@name|name|U...>`
- `--after YYYY-MM-DD`
- `--before YYYY-MM-DD`
- `--content-type any|text|image|snippet|file`
- `--limit <n>` (default `20`)
- `--max-content-chars <n>` (default `4000`, `-1` unlimited; messages only)

## Messages — additional commands

- `slackers message history <channel>`
  - Fetch full channel history (all pages, resumable)
  - Options:
    - `--workspace <url>`
    - `--limit <n>` (default `500`)
    - `--after YYYY-MM-DD` / `--before YYYY-MM-DD`
    - `--max-body-chars <n>` (default `8000`, `-1` unlimited)
    - `--include-threads` (inline thread replies)
    - `--include-reactions`
    - `--output <file>` / `-o <file>` (default `<channel>-history.json`)

- `slackers message thread-participants [<url>]`
  - List unique participants in a thread with message counts
  - Options:
    - `--channel <C...>` + `--ts <ts>` (when not using a URL)
    - `--workspace <url>`
    - `--resolve-users`

- `slackers message pin --channel <C...> --ts <ts> [--workspace <url>]`
- `slackers message unpin --channel <C...> --ts <ts> [--workspace <url>]`
- `slackers message delete --channel <C...> --ts <ts> [--workspace <url>]`
- `slackers message update --channel <C...> --ts <ts> --text <text> [--workspace <url>]`

## Canvas

- `slackers canvas get <canvas-url-or-id>`
  - Options:
    - `--workspace <url>` (required when passing an id and multiple workspaces)
    - `--max-chars <n>` (default `20000`, `-1` unlimited)

## Users

- `slackers user list [--workspace <url>] [--limit <n>] [--cursor <cursor>] [--include-bots]`
- `slackers user get <U...|@handle|handle> [--workspace <url>]`

## Channels — additional commands

- `slackers channel list`
  - Additional options:
    - `--resolve-users` (enrich DM listings with display names)

- `slackers channel mark <target> --ts <ts> [--workspace <url>]`
  - Mark a channel/DM as read up to the given message timestamp

- `slackers channel members <target> [--resolve-users] [--workspace <url>]`
  - List member user IDs (and optional display names) for a channel

- `slackers channel new --name <name> [--private] [--workspace <url>]`
  - Create a new public or private channel

- `slackers channel invite <target> --users <U1,U2,...> [--workspace <url>]`
  - Invite one or more users to a channel

## Files

- `slackers file upload --file <path> [--channels <C1,C2>] [--comment <text>] [--title <title>] [--filename <name>] [--workspace <url>]`
- `slackers file delete --file-id <F...> [--workspace <url>]`
- `slackers file list [--channel <C...>] [--limit <n>] [--workspace <url>]`

## Workspace

- `slackers workspace info [--workspace <url>]`
  - Returns team id, name, domain, and icon URL

## Emoji

- `slackers emoji list [--workspace <url>]`
  - Lists all custom emoji for the workspace

## Direct Messages

- `slackers dm open --users <U1,U2,...> [--workspace <url>]`
  - Open a DM (or MPIM) conversation with one or more users; returns channel ID

- `slackers dm send --users <U1,U2,...> --message <text> [--workspace <url>]`
  - Open a DM and send a message in one step

## Batch Operations

- `slackers batch send --message <text> --channels <C1,C2,...> [--workspace <url>]`
  - Send the same message to multiple channels

- `slackers batch react --emoji <name> --messages <url1,url2,...>`
  - Add a reaction to multiple messages

## Later (Saved / Starred Messages)

- `slackers later add --channel <C...> --ts <ts> [--workspace <url>]`
- `slackers later remove --channel <C...> --ts <ts> [--workspace <url>]`
- `slackers later list [--limit <n>] [--workspace <url>]`

## Scheduled Messages

- `slackers scheduled send --channel <C...> --message <text> --at <unix-ts|RFC3339> [--workspace <url>]`
- `slackers scheduled list [--channel <C...>] [--workspace <url>]`
- `slackers scheduled delete --channel <C...> --id <scheduled-message-id> [--workspace <url>]`

## Mentions

- `slackers mention list [--username <handle>] [--channel <channel...>] [--after YYYY-MM-DD] [--before YYYY-MM-DD] [--limit <n>] [--workspace <url>]`
  - List messages that @mention you (or a named user)

## Export

- `slackers export channel --channel <C...|#name> [--format json|csv|html] [--output <file>] [--workspace <url>]`
  - Export channel history in the requested format (default: json)

## Output Format Flag

Most list/tabular commands support `--format <fmt>` to control rendering:

- `json` (default) — pretty-printed JSON with empty fields pruned
- `table` — ASCII table via comfy-table
- `markdown` — GitHub-flavoured Markdown table
- `plain` — tab-separated or `key=value` lines

```bash
slackers message list <url> --format table
slackers channel list --format markdown
slackers search messages "deploy" --format json
```
