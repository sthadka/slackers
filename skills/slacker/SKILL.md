---
name: slacker
description: |
  Slack automation CLI for AI agents. Use when:
  - Reading a Slack message or thread (given a URL or channel+ts)
  - Downloading Slack attachments (snippets, images, files) to local paths
  - Searching Slack messages or files
  - Sending a reply or adding a reaction
  - Fetching a Slack canvas as markdown
  - Looking up Slack users
  Triggers: "slack message", "slack thread", "slack URL", "slack link", "read slack", "reply on slack", "search slack"
---

# Slack automation with `slackers`

`slackers` is a CLI binary installed on `$PATH`. Invoke it directly (e.g. `slackers user list`)

## Quick start (auth)

Authentication is automatic on macOS (Slack Desktop first, then Chrome fallback).

If credentials aren't available, run one of:

- Slack Desktop (default):

```bash
slackers auth import-desktop
slackers auth test
```

- Chrome fallback:

```bash
slackers auth import-chrome
slackers auth test
```

- Or set env vars (browser tokens; avoid pasting these into chat logs):

```bash
export SLACK_TOKEN="xoxc-..."
export SLACK_COOKIE_D="xoxd-..."
slackers auth test
```

- Or set a standard token:

```bash
export SLACK_TOKEN="xoxb-..."  # or xoxp-...
slackers auth test
```

Check configured workspaces:

```bash
slackers auth whoami
```

## Canonical workflow (given a Slack message URL)

1. Fetch a single message (plus thread summary, if any):

```bash
slackers message get "https://workspace.slack.com/archives/C123/p1700000000000000"
```

2. If you need the full thread:

```bash
slackers message list "https://workspace.slack.com/archives/C123/p1700000000000000"
```

## Attachments (snippets/images/files)

`message get/list` and `search` auto-download attachments and include absolute paths in JSON output (typically under `message.files[].path` / `files[].path`).

## Reply or react (does the right thing)

```bash
slackers message send "https://workspace.slack.com/archives/C123/p1700000000000000" "I can take this."
slackers message react "https://workspace.slack.com/archives/C123/p1700000000000000" "eyes"
```

## Search (messages + files)

Prefer channel-scoped search for reliability:

```bash
slackers search all "smoke tests failed" --channel "#alerts" --after 2026-01-01 --before 2026-02-01
slackers search messages "stably test" --user "@alice" --channel general
slackers search files "testing" --content-type snippet --limit 10
```

## Multi-workspace guardrail (important)

If you have multiple workspaces configured and you use a channel **name** (`#general` / `general`), pass `--workspace` (or set `SLACK_WORKSPACE_URL`) to avoid ambiguity:

```bash
slackers message get "#general" --workspace "https://myteam.slack.com" --ts "1770165109.628379"
```

## Canvas + Users

```bash
slackers canvas get "https://workspace.slack.com/docs/T123/F456"
slackers user list --workspace "https://workspace.slack.com" --limit 100
slackers user get "@alice" --workspace "https://workspace.slack.com"
```

## References

- [references/commands.md](references/commands.md): full command map + all flags
- [references/targets.md](references/targets.md): URL vs `#channel` targeting rules
- [references/output.md](references/output.md): JSON output shapes + download paths
