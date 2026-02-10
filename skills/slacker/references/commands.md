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

## Canvas

- `slackers canvas get <canvas-url-or-id>`
  - Options:
    - `--workspace <url>` (required when passing an id and multiple workspaces)
    - `--max-chars <n>` (default `20000`, `-1` unlimited)

## Users

- `slackers user list [--workspace <url>] [--limit <n>] [--cursor <cursor>] [--include-bots]`
- `slackers user get <U...|@handle|handle> [--workspace <url>]`
