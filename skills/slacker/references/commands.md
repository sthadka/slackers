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
  - Fetches the full thread
  - Options:
    - `--workspace <url>` (same rules as above)
    - `--thread-ts <seconds>.<micros>` (required for channel targets unless you pass `--ts`)
    - `--ts <seconds>.<micros>` (optional: resolve a message to its thread)
    - `--max-body-chars <n>` (default `8000`, `-1` unlimited)
    - `--include-reactions`

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
