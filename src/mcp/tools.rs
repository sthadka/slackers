use serde_json::{json, Value};

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub is_write: bool,
}

fn prop(desc: &str) -> Value {
    json!({"type": "string", "description": desc})
}

fn int_prop(desc: &str) -> Value {
    json!({"type": "integer", "description": desc})
}

fn bool_prop(desc: &str) -> Value {
    json!({"type": "boolean", "description": desc})
}

fn arr_prop(desc: &str) -> Value {
    json!({"type": "array", "items": {"type": "string"}, "description": desc})
}

pub fn all_tools() -> Vec<ToolDef> {
    vec![
        // ---- Message (read) ----
        ToolDef {
            name: "message_get",
            description: "Fetch a single Slack message by URL, #channel+ts, or channel ID+ts",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Slack message URL, #channel, or channel ID"),
                    "ts": prop("Message timestamp (required when using #channel/channel ID)"),
                    "thread_ts": prop("Thread root ts hint"),
                    "workspace": prop("Workspace URL"),
                    "max_body_chars": int_prop("Max content chars (default 8000, -1 unlimited)"),
                    "include_reactions": bool_prop("Include reactions"),
                    "resolve_users": bool_prop("Resolve user IDs to display names"),
                    "refresh_users": bool_prop("Force refresh user cache")
                },
                "required": ["target"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "message_list",
            description: "List thread replies for a Slack message, or recent messages in a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Slack message URL, #channel, or channel ID"),
                    "thread_ts": prop("Thread root ts"),
                    "ts": prop("Message ts (resolve to its thread)"),
                    "workspace": prop("Workspace URL"),
                    "format": prop("Output format: json, table, markdown, plain"),
                    "max_body_chars": int_prop("Max content chars (default 8000, -1 unlimited)"),
                    "include_reactions": bool_prop("Include reactions"),
                    "limit": int_prop("Max messages to return (default 100)"),
                    "after_ts": prop("Only messages after this timestamp"),
                    "before_ts": prop("Only messages before this timestamp"),
                    "user": prop("Filter by user ID or @handle"),
                    "has_link": bool_prop("Only messages with links"),
                    "has_file": bool_prop("Only messages with files"),
                    "has_reaction": bool_prop("Only messages with reactions"),
                    "with_reaction": prop("Only messages with this reaction emoji"),
                    "without_reaction": prop("Only messages without this reaction emoji"),
                    "resolve_users": bool_prop("Resolve user IDs to display names"),
                    "refresh_users": bool_prop("Force refresh user cache")
                },
                "required": ["target"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "message_history",
            description: "Fetch all messages from a channel with optional thread expansion",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel name (#name) or ID (C...)"),
                    "workspace": prop("Workspace URL"),
                    "limit": int_prop("Max top-level messages (default 500)"),
                    "after": prop("Only messages after YYYY-MM-DD"),
                    "before": prop("Only messages before YYYY-MM-DD"),
                    "max_body_chars": int_prop("Max body chars (default 8000, -1 unlimited)"),
                    "include_threads": bool_prop("Fetch and inline thread replies"),
                    "include_reactions": bool_prop("Include reactions"),
                    "output": prop("Output file path")
                },
                "required": ["channel"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "message_thread_participants",
            description: "List unique participants in a thread with message counts",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Slack thread URL"),
                    "channel": prop("Channel ID (when not using URL)"),
                    "ts": prop("Thread root ts (when not using URL)"),
                    "workspace": prop("Workspace URL"),
                    "resolve_users": bool_prop("Resolve user IDs to display names")
                }
            }),
            is_write: false,
        },
        // ---- Message (write) ----
        ToolDef {
            name: "message_send",
            description: "Send a message to a channel or thread",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Slack message URL, #name/name, or channel ID"),
                    "text": prop("Message text"),
                    "workspace": prop("Workspace URL"),
                    "thread_ts": prop("Thread root ts to reply in"),
                    "reply_broadcast": bool_prop("Broadcast threaded reply to channel"),
                    "blocks": prop("JSON string of Block Kit blocks array")
                },
                "required": ["target", "text"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "message_update",
            description: "Update the text of an existing message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...)"),
                    "ts": prop("Message timestamp"),
                    "text": prop("New text for the message"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "ts", "text"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "message_delete",
            description: "Delete a message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...)"),
                    "ts": prop("Message timestamp"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "ts"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "message_pin",
            description: "Pin a message to a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...)"),
                    "ts": prop("Message timestamp"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "ts"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "message_unpin",
            description: "Unpin a message from a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...)"),
                    "ts": prop("Message timestamp"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "ts"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "react_add",
            description: "Add an emoji reaction to a message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Slack message URL, #channel, or channel ID"),
                    "emoji": prop("Emoji name (e.g. rocket, thumbsup)"),
                    "workspace": prop("Workspace URL"),
                    "ts": prop("Message ts (when using #channel/channel ID)")
                },
                "required": ["target", "emoji"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "react_remove",
            description: "Remove an emoji reaction from a message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Slack message URL, #channel, or channel ID"),
                    "emoji": prop("Emoji name (e.g. rocket, thumbsup)"),
                    "workspace": prop("Workspace URL"),
                    "ts": prop("Message ts (when using #channel/channel ID)")
                },
                "required": ["target", "emoji"]
            }),
            is_write: true,
        },
        // ---- Search ----
        ToolDef {
            name: "search_messages",
            description: "Search Slack messages by query with filters",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": prop("Search query string"),
                    "workspace": prop("Workspace URL"),
                    "format": prop("Output format: json, table, markdown, plain"),
                    "channel": arr_prop("Channel filter (#name, name, or ID)"),
                    "user": prop("User filter (@name, name, or U...)"),
                    "after": prop("Only after YYYY-MM-DD"),
                    "before": prop("Only before YYYY-MM-DD"),
                    "limit": int_prop("Max results (default 20)"),
                    "max_content_chars": int_prop("Max message chars (default 4000)"),
                    "sort": prop("Sort: timestamp or relevance"),
                    "content_type": prop("Filter by content type: any, text, image, snippet, file"),
                    "has_link": bool_prop("Only with links"),
                    "has_emoji": bool_prop("Only with reactions"),
                    "from_me": bool_prop("Only from authenticated user"),
                    "resolve_users": bool_prop("Resolve user IDs"),
                    "refresh_users": bool_prop("Force refresh user cache")
                },
                "required": ["query"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "search_files",
            description: "Search Slack files by query with filters",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": prop("Search query string"),
                    "workspace": prop("Workspace URL"),
                    "format": prop("Output format: json, table, markdown, plain"),
                    "channel": arr_prop("Channel filter"),
                    "user": prop("User filter"),
                    "after": prop("Only after YYYY-MM-DD"),
                    "before": prop("Only before YYYY-MM-DD"),
                    "limit": int_prop("Max results (default 20)"),
                    "max_content_chars": int_prop("Max content chars (default 4000)"),
                    "sort": prop("Sort: timestamp or relevance"),
                    "content_type": prop("Filter by content type: any, text, image, snippet, file"),
                    "has_link": bool_prop("Only with links"),
                    "has_emoji": bool_prop("Only with reactions"),
                    "from_me": bool_prop("Only from authenticated user"),
                    "resolve_users": bool_prop("Resolve user IDs"),
                    "refresh_users": bool_prop("Force refresh user cache")
                },
                "required": ["query"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "search_all",
            description: "Search Slack messages and files by query",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": prop("Search query string"),
                    "workspace": prop("Workspace URL"),
                    "format": prop("Output format: json, table, markdown, plain"),
                    "channel": arr_prop("Channel filter"),
                    "user": prop("User filter"),
                    "after": prop("Only after YYYY-MM-DD"),
                    "before": prop("Only before YYYY-MM-DD"),
                    "limit": int_prop("Max results (default 20)"),
                    "max_content_chars": int_prop("Max content chars (default 4000)"),
                    "sort": prop("Sort: timestamp or relevance"),
                    "content_type": prop("Filter by content type: any, text, image, snippet, file"),
                    "has_link": bool_prop("Only with links"),
                    "has_emoji": bool_prop("Only with reactions"),
                    "from_me": bool_prop("Only from authenticated user"),
                    "resolve_users": bool_prop("Resolve user IDs"),
                    "refresh_users": bool_prop("Force refresh user cache")
                },
                "required": ["query"]
            }),
            is_write: false,
        },
        // ---- Canvas ----
        ToolDef {
            name: "canvas_get",
            description: "Fetch a Slack canvas and convert it to Markdown",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "canvas": prop("Slack canvas URL or canvas ID (F...)"),
                    "workspace": prop("Workspace URL"),
                    "max_chars": int_prop("Max markdown chars (default 20000, -1 unlimited)")
                },
                "required": ["canvas"]
            }),
            is_write: false,
        },
        // ---- User ----
        ToolDef {
            name: "user_list",
            description: "List users in the workspace",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspace": prop("Workspace URL"),
                    "limit": int_prop("Max users (default 200)"),
                    "include_bots": bool_prop("Include bot users"),
                    "format": prop("Output format: json, table, markdown, plain")
                }
            }),
            is_write: false,
        },
        ToolDef {
            name: "user_get",
            description: "Get a single user by ID (U...) or @handle",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "user": prop("User ID (U...) or @handle"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["user"]
            }),
            is_write: false,
        },
        // ---- Channel (read) ----
        ToolDef {
            name: "channel_list",
            description: "List channels/conversations in the workspace",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspace": prop("Workspace URL"),
                    "types": arr_prop("Conversation types (public_channel, private_channel, mpim, im)"),
                    "exclude_archived": bool_prop("Exclude archived channels (default true)"),
                    "limit": int_prop("Max channels (default 200)"),
                    "resolve_users": bool_prop("Resolve user IDs for DMs"),
                    "all": bool_prop("Show all channels, not just joined"),
                    "format": prop("Output format: json, table, markdown, plain")
                }
            }),
            is_write: false,
        },
        ToolDef {
            name: "channel_get",
            description: "Get detailed information about a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...) or #name/name"),
                    "workspace": prop("Workspace URL"),
                    "include_num_members": bool_prop("Include member count")
                },
                "required": ["channel"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "channel_members",
            description: "List members of a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Channel ID (C...) or #name/name"),
                    "resolve_users": bool_prop("Resolve user IDs to display names"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["target"]
            }),
            is_write: false,
        },
        // ---- Channel (write) ----
        ToolDef {
            name: "channel_join",
            description: "Join a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...) or #name/name"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "channel_leave",
            description: "Leave a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...) or #name/name"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "channel_mark",
            description: "Mark a channel or DM as read up to a given message timestamp",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Channel ID (C...), DM ID (D...), or #name"),
                    "ts": prop("Message timestamp to mark as read up to"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["target", "ts"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "channel_new",
            description: "Create a new channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": prop("Channel name (lowercase, hyphens, underscores)"),
                    "private": bool_prop("Create as private channel"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["name"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "channel_invite",
            description: "Invite users to a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": prop("Channel ID (C...) or #name/name"),
                    "users": arr_prop("User IDs to invite"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["target", "users"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "channel_rename",
            description: "Rename a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...) or #name/name"),
                    "name": prop("New name for the channel"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "name"]
            }),
            is_write: true,
        },
        // ---- Batch (write) ----
        ToolDef {
            name: "batch_send",
            description: "Send a message to multiple channels at once",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": prop("Message text to send"),
                    "channels": arr_prop("Channels (e.g. #general, C123)"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["message", "channels"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "batch_react",
            description: "Add a reaction to multiple messages at once",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "emoji": prop("Emoji name to react with"),
                    "messages": arr_prop("Slack message URLs")
                },
                "required": ["emoji", "messages"]
            }),
            is_write: true,
        },
        // ---- File ----
        ToolDef {
            name: "file_list",
            description: "List files in the workspace or a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Filter by channel ID"),
                    "limit": int_prop("Max files (default 100)"),
                    "workspace": prop("Workspace URL")
                }
            }),
            is_write: false,
        },
        ToolDef {
            name: "file_upload",
            description: "Upload a file to Slack",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": prop("Local path of the file to upload"),
                    "channels": arr_prop("Channel IDs or names to share into"),
                    "comment": prop("Initial comment"),
                    "title": prop("Display title"),
                    "filename": prop("Override filename"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["file"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "file_delete",
            description: "Delete a file by ID",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_id": prop("Slack file ID (F...)"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["file_id"]
            }),
            is_write: true,
        },
        // ---- Workspace ----
        ToolDef {
            name: "workspace_info",
            description: "Fetch workspace information (name, domain, icon)",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspace": prop("Workspace URL")
                }
            }),
            is_write: false,
        },
        // ---- Emoji ----
        ToolDef {
            name: "emoji_list",
            description: "List all custom emoji for the workspace",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workspace": prop("Workspace URL")
                }
            }),
            is_write: false,
        },
        // ---- Later ----
        ToolDef {
            name: "later_list",
            description: "List starred (saved for later) items",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": int_prop("Max items (default 100)"),
                    "workspace": prop("Workspace URL")
                }
            }),
            is_write: false,
        },
        ToolDef {
            name: "later_add",
            description: "Star (save for later) a message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID"),
                    "ts": prop("Message timestamp"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "ts"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "later_remove",
            description: "Unstar (remove from saved) a message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID"),
                    "ts": prop("Message timestamp"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "ts"]
            }),
            is_write: true,
        },
        // ---- Scheduled ----
        ToolDef {
            name: "scheduled_send",
            description: "Schedule a message to be sent at a future time",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID or name"),
                    "message": prop("Message text"),
                    "at": prop("Unix timestamp or RFC3339 datetime"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "message", "at"]
            }),
            is_write: true,
        },
        ToolDef {
            name: "scheduled_list",
            description: "List scheduled messages",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Filter by channel"),
                    "workspace": prop("Workspace URL")
                }
            }),
            is_write: false,
        },
        ToolDef {
            name: "scheduled_delete",
            description: "Delete a scheduled message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID"),
                    "id": prop("Scheduled message ID"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "id"]
            }),
            is_write: true,
        },
        // ---- DM ----
        ToolDef {
            name: "dm_open",
            description: "Open a direct message conversation with one or more users",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "users": arr_prop("User IDs to open DM with"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["users"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "dm_send",
            description: "Open a DM and send a message",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "users": arr_prop("User IDs to DM"),
                    "message": prop("Message text to send"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["users", "message"]
            }),
            is_write: true,
        },
        // ---- Mention ----
        ToolDef {
            name: "mention_list",
            description: "List messages that @mention you or a named user",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "username": prop("Username to search mentions of (defaults to authenticated user)"),
                    "channel": arr_prop("Channel filter (#name, name, or ID)"),
                    "after": prop("Only after YYYY-MM-DD"),
                    "before": prop("Only before YYYY-MM-DD"),
                    "limit": int_prop("Max results (default 20)"),
                    "workspace": prop("Workspace URL")
                }
            }),
            is_write: false,
        },
        // ---- Export ----
        ToolDef {
            name: "export_channel",
            description: "Export channel history in json, csv, or html format",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID (C...) or #name/name"),
                    "format": prop("Output format: json, csv, html (default json)"),
                    "output": prop("Output file path (defaults to stdout)"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel"]
            }),
            is_write: false,
        },
        // ---- Unreads ----
        ToolDef {
            name: "unreads_show",
            description: "Show unread messages across all conversations",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "counts_only": bool_prop("Only show counts, not message content"),
                    "max_messages": int_prop("Max unread messages per channel (default 10)"),
                    "max_body_chars": int_prop("Max chars per message (default 4000, -1 unlimited)"),
                    "include_system": bool_prop("Include system messages"),
                    "format": prop("Output format: json, table, markdown, plain"),
                    "workspace": prop("Workspace URL")
                }
            }),
            is_write: false,
        },
        // ---- Workflow ----
        ToolDef {
            name: "workflow_list",
            description: "List workflows bookmarked or featured in a channel",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID or name"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "workflow_preview",
            description: "Get workflow metadata from a trigger ID (no side effects)",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "trigger_id": prop("Trigger ID (Ft...)"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["trigger_id"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "workflow_get",
            description: "Get workflow definition including form fields and steps",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": prop("Trigger ID (Ft...) or Workflow ID (Wf...)"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["id"]
            }),
            is_write: false,
        },
        ToolDef {
            name: "workflow_run",
            description: "Trip a workflow trigger",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "trigger_id": prop("Trigger ID (Ft...)"),
                    "channel": prop("Channel where the workflow is bookmarked"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["trigger_id", "channel"]
            }),
            is_write: true,
        },
        // ---- Slash ----
        ToolDef {
            name: "slash_run",
            description: "Execute a slash command in a channel (requires browser token)",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "channel": prop("Channel ID or #name"),
                    "command": prop("Slash command with args (e.g. /remind me to check in 1 hour)"),
                    "workspace": prop("Workspace URL")
                },
                "required": ["channel", "command"]
            }),
            is_write: true,
        },
    ]
}

fn str_val(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn str_req(args: &Value, key: &str) -> String {
    str_val(args, key).unwrap_or_default()
}

fn bool_val(args: &Value, key: &str) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn int_val(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn str_arr(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

pub fn tool_to_cli_args(name: &str, args: &Value) -> Option<Vec<String>> {
    let mut cli: Vec<String> = Vec::new();

    match name {
        "message_get" => {
            cli.push("message".into());
            cli.push("get".into());
            cli.push(str_req(args, "target"));
            if let Some(v) = str_val(args, "ts") { cli.push("--ts".into()); cli.push(v); }
            if let Some(v) = str_val(args, "thread_ts") { cli.push("--thread-ts".into()); cli.push(v); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = int_val(args, "max_body_chars") { cli.push("--max-body-chars".into()); cli.push(v.to_string()); }
            if bool_val(args, "include_reactions") { cli.push("--include-reactions".into()); }
            if bool_val(args, "resolve_users") { cli.push("--resolve-users".into()); }
            if bool_val(args, "refresh_users") { cli.push("--refresh-users".into()); }
        }
        "message_list" => {
            cli.push("message".into());
            cli.push("list".into());
            cli.push(str_req(args, "target"));
            if let Some(v) = str_val(args, "thread_ts") { cli.push("--thread-ts".into()); cli.push(v); }
            if let Some(v) = str_val(args, "ts") { cli.push("--ts".into()); cli.push(v); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = str_val(args, "format") { cli.push("--format".into()); cli.push(v); }
            if let Some(v) = int_val(args, "max_body_chars") { cli.push("--max-body-chars".into()); cli.push(v.to_string()); }
            if bool_val(args, "include_reactions") { cli.push("--include-reactions".into()); }
            if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
            if let Some(v) = str_val(args, "after_ts") { cli.push("--after-ts".into()); cli.push(v); }
            if let Some(v) = str_val(args, "before_ts") { cli.push("--before-ts".into()); cli.push(v); }
            if let Some(v) = str_val(args, "user") { cli.push("--user".into()); cli.push(v); }
            if bool_val(args, "has_link") { cli.push("--has-link".into()); }
            if bool_val(args, "has_file") { cli.push("--has-file".into()); }
            if bool_val(args, "has_reaction") { cli.push("--has-reaction".into()); }
            if let Some(v) = str_val(args, "with_reaction") { cli.push("--with-reaction".into()); cli.push(v); }
            if let Some(v) = str_val(args, "without_reaction") { cli.push("--without-reaction".into()); cli.push(v); }
            if bool_val(args, "resolve_users") { cli.push("--resolve-users".into()); }
            if bool_val(args, "refresh_users") { cli.push("--refresh-users".into()); }
        }
        "message_history" => {
            cli.push("message".into());
            cli.push("history".into());
            cli.push(str_req(args, "channel"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
            if let Some(v) = str_val(args, "after") { cli.push("--after".into()); cli.push(v); }
            if let Some(v) = str_val(args, "before") { cli.push("--before".into()); cli.push(v); }
            if let Some(v) = int_val(args, "max_body_chars") { cli.push("--max-body-chars".into()); cli.push(v.to_string()); }
            if bool_val(args, "include_threads") { cli.push("--include-threads".into()); }
            if bool_val(args, "include_reactions") { cli.push("--include-reactions".into()); }
            if let Some(v) = str_val(args, "output") { cli.push("--output".into()); cli.push(v); }
        }
        "message_thread_participants" => {
            cli.push("message".into());
            cli.push("thread-participants".into());
            if let Some(v) = str_val(args, "target") { cli.push(v); }
            if let Some(v) = str_val(args, "channel") { cli.push("--channel".into()); cli.push(v); }
            if let Some(v) = str_val(args, "ts") { cli.push("--ts".into()); cli.push(v); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if bool_val(args, "resolve_users") { cli.push("--resolve-users".into()); }
        }
        "message_send" => {
            cli.push("message".into());
            cli.push("send".into());
            cli.push(str_req(args, "target"));
            cli.push(str_req(args, "text"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = str_val(args, "thread_ts") { cli.push("--thread-ts".into()); cli.push(v); }
            if bool_val(args, "reply_broadcast") { cli.push("--reply-broadcast".into()); }
            if let Some(v) = str_val(args, "blocks") { cli.push("--blocks".into()); cli.push(v); }
        }
        "message_update" => {
            cli.push("message".into());
            cli.push("update".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--ts".into()); cli.push(str_req(args, "ts"));
            cli.push("--text".into()); cli.push(str_req(args, "text"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "message_delete" => {
            cli.push("message".into());
            cli.push("delete".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--ts".into()); cli.push(str_req(args, "ts"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "message_pin" => {
            cli.push("message".into());
            cli.push("pin".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--ts".into()); cli.push(str_req(args, "ts"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "message_unpin" => {
            cli.push("message".into());
            cli.push("unpin".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--ts".into()); cli.push(str_req(args, "ts"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "react_add" => {
            cli.push("message".into());
            cli.push("react".into());
            cli.push("add".into());
            cli.push(str_req(args, "target"));
            cli.push(str_req(args, "emoji"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = str_val(args, "ts") { cli.push("--ts".into()); cli.push(v); }
        }
        "react_remove" => {
            cli.push("message".into());
            cli.push("react".into());
            cli.push("remove".into());
            cli.push(str_req(args, "target"));
            cli.push(str_req(args, "emoji"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = str_val(args, "ts") { cli.push("--ts".into()); cli.push(v); }
        }
        "search_messages" => {
            cli.push("search".into());
            cli.push("messages".into());
            cli.push(str_req(args, "query"));
            append_search_opts(&mut cli, args);
        }
        "search_files" => {
            cli.push("search".into());
            cli.push("files".into());
            cli.push(str_req(args, "query"));
            append_search_opts(&mut cli, args);
        }
        "search_all" => {
            cli.push("search".into());
            cli.push("all".into());
            cli.push(str_req(args, "query"));
            append_search_opts(&mut cli, args);
        }
        "canvas_get" => {
            cli.push("canvas".into());
            cli.push("get".into());
            cli.push(str_req(args, "canvas"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = int_val(args, "max_chars") { cli.push("--max-chars".into()); cli.push(v.to_string()); }
        }
        "user_list" => {
            cli.push("user".into());
            cli.push("list".into());
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
            if bool_val(args, "include_bots") { cli.push("--include-bots".into()); }
            if let Some(v) = str_val(args, "format") { cli.push("--format".into()); cli.push(v); }
        }
        "user_get" => {
            cli.push("user".into());
            cli.push("get".into());
            cli.push(str_req(args, "user"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "channel_list" => {
            cli.push("channel".into());
            cli.push("list".into());
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            for t in str_arr(args, "types") { cli.push("--types".into()); cli.push(t); }
            if let Some(v) = args.get("exclude_archived") {
                if let Some(b) = v.as_bool() {
                    cli.push("--exclude-archived".into()); cli.push(b.to_string());
                }
            }
            if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
            if bool_val(args, "resolve_users") { cli.push("--resolve-users".into()); }
            if bool_val(args, "all") { cli.push("--all".into()); }
            if let Some(v) = str_val(args, "format") { cli.push("--format".into()); cli.push(v); }
        }
        "channel_get" => {
            cli.push("channel".into());
            cli.push("get".into());
            cli.push(str_req(args, "channel"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
            if bool_val(args, "include_num_members") { cli.push("--include-num-members".into()); }
        }
        "channel_members" => {
            cli.push("channel".into());
            cli.push("members".into());
            cli.push(str_req(args, "target"));
            if bool_val(args, "resolve_users") { cli.push("--resolve-users".into()); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "channel_join" => {
            cli.push("channel".into());
            cli.push("join".into());
            cli.push(str_req(args, "channel"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "channel_leave" => {
            cli.push("channel".into());
            cli.push("leave".into());
            cli.push(str_req(args, "channel"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "channel_mark" => {
            cli.push("channel".into());
            cli.push("mark".into());
            cli.push(str_req(args, "target"));
            cli.push("--ts".into()); cli.push(str_req(args, "ts"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "channel_new" => {
            cli.push("channel".into());
            cli.push("new".into());
            cli.push("--name".into()); cli.push(str_req(args, "name"));
            if bool_val(args, "private") { cli.push("--private".into()); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "channel_invite" => {
            cli.push("channel".into());
            cli.push("invite".into());
            cli.push(str_req(args, "target"));
            let users = str_arr(args, "users");
            if !users.is_empty() {
                cli.push("--users".into());
                cli.push(users.join(","));
            }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "channel_rename" => {
            cli.push("channel".into());
            cli.push("rename".into());
            cli.push(str_req(args, "channel"));
            cli.push(str_req(args, "name"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "batch_send" => {
            cli.push("batch".into());
            cli.push("send".into());
            cli.push("--message".into()); cli.push(str_req(args, "message"));
            let channels = str_arr(args, "channels");
            if !channels.is_empty() {
                cli.push("--channels".into());
                cli.push(channels.join(","));
            }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "batch_react" => {
            cli.push("batch".into());
            cli.push("react".into());
            cli.push("--emoji".into()); cli.push(str_req(args, "emoji"));
            let messages = str_arr(args, "messages");
            if !messages.is_empty() {
                cli.push("--messages".into());
                cli.push(messages.join(","));
            }
        }
        "file_list" => {
            cli.push("file".into());
            cli.push("list".into());
            if let Some(v) = str_val(args, "channel") { cli.push("--channel".into()); cli.push(v); }
            if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "file_upload" => {
            cli.push("file".into());
            cli.push("upload".into());
            cli.push("--file".into()); cli.push(str_req(args, "file"));
            let channels = str_arr(args, "channels");
            if !channels.is_empty() {
                cli.push("--channels".into());
                cli.push(channels.join(","));
            }
            if let Some(v) = str_val(args, "comment") { cli.push("--comment".into()); cli.push(v); }
            if let Some(v) = str_val(args, "title") { cli.push("--title".into()); cli.push(v); }
            if let Some(v) = str_val(args, "filename") { cli.push("--filename".into()); cli.push(v); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "file_delete" => {
            cli.push("file".into());
            cli.push("delete".into());
            cli.push("--file-id".into()); cli.push(str_req(args, "file_id"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "workspace_info" => {
            cli.push("workspace".into());
            cli.push("info".into());
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "emoji_list" => {
            cli.push("emoji".into());
            cli.push("list".into());
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "later_list" => {
            cli.push("later".into());
            cli.push("list".into());
            if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "later_add" => {
            cli.push("later".into());
            cli.push("add".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--ts".into()); cli.push(str_req(args, "ts"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "later_remove" => {
            cli.push("later".into());
            cli.push("remove".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--ts".into()); cli.push(str_req(args, "ts"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "scheduled_send" => {
            cli.push("scheduled".into());
            cli.push("send".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--message".into()); cli.push(str_req(args, "message"));
            cli.push("--at".into()); cli.push(str_req(args, "at"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "scheduled_list" => {
            cli.push("scheduled".into());
            cli.push("list".into());
            if let Some(v) = str_val(args, "channel") { cli.push("--channel".into()); cli.push(v); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "scheduled_delete" => {
            cli.push("scheduled".into());
            cli.push("delete".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            cli.push("--id".into()); cli.push(str_req(args, "id"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "dm_open" => {
            cli.push("dm".into());
            cli.push("open".into());
            let users = str_arr(args, "users");
            if !users.is_empty() {
                cli.push("--users".into());
                cli.push(users.join(","));
            }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "dm_send" => {
            cli.push("dm".into());
            cli.push("send".into());
            let users = str_arr(args, "users");
            if !users.is_empty() {
                cli.push("--users".into());
                cli.push(users.join(","));
            }
            cli.push("--message".into()); cli.push(str_req(args, "message"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "mention_list" => {
            cli.push("mention".into());
            cli.push("list".into());
            if let Some(v) = str_val(args, "username") { cli.push("--username".into()); cli.push(v); }
            for ch in str_arr(args, "channel") { cli.push("--channel".into()); cli.push(ch); }
            if let Some(v) = str_val(args, "after") { cli.push("--after".into()); cli.push(v); }
            if let Some(v) = str_val(args, "before") { cli.push("--before".into()); cli.push(v); }
            if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "export_channel" => {
            cli.push("export".into());
            cli.push("channel".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            if let Some(v) = str_val(args, "format") { cli.push("--format".into()); cli.push(v); }
            if let Some(v) = str_val(args, "output") { cli.push("--output".into()); cli.push(v); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "unreads_show" => {
            cli.push("unreads".into());
            cli.push("show".into());
            if bool_val(args, "counts_only") { cli.push("--counts-only".into()); }
            if let Some(v) = int_val(args, "max_messages") { cli.push("--max-messages".into()); cli.push(v.to_string()); }
            if let Some(v) = int_val(args, "max_body_chars") { cli.push("--max-body-chars".into()); cli.push(v.to_string()); }
            if bool_val(args, "include_system") { cli.push("--include-system".into()); }
            if let Some(v) = str_val(args, "format") { cli.push("--format".into()); cli.push(v); }
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "workflow_list" => {
            cli.push("workflow".into());
            cli.push("list".into());
            cli.push(str_req(args, "channel"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "workflow_preview" => {
            cli.push("workflow".into());
            cli.push("preview".into());
            cli.push(str_req(args, "trigger_id"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "workflow_get" => {
            cli.push("workflow".into());
            cli.push("get".into());
            cli.push(str_req(args, "id"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "workflow_run" => {
            cli.push("workflow".into());
            cli.push("run".into());
            cli.push(str_req(args, "trigger_id"));
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
        }
        "slash_run" => {
            cli.push("slash".into());
            cli.push("run".into());
            cli.push("--channel".into()); cli.push(str_req(args, "channel"));
            let cmd = str_req(args, "command");
            for part in cmd.split_whitespace() {
                cli.push(part.to_string());
            }
        }
        _ => return None,
    }

    Some(cli)
}

fn append_search_opts(cli: &mut Vec<String>, args: &Value) {
    if let Some(v) = str_val(args, "workspace") { cli.push("--workspace".into()); cli.push(v); }
    if let Some(v) = str_val(args, "format") { cli.push("--format".into()); cli.push(v); }
    for ch in str_arr(args, "channel") { cli.push("--channel".into()); cli.push(ch); }
    if let Some(v) = str_val(args, "user") { cli.push("--user".into()); cli.push(v); }
    if let Some(v) = str_val(args, "after") { cli.push("--after".into()); cli.push(v); }
    if let Some(v) = str_val(args, "before") { cli.push("--before".into()); cli.push(v); }
    if let Some(v) = str_val(args, "content_type") { cli.push("--content-type".into()); cli.push(v); }
    if let Some(v) = int_val(args, "limit") { cli.push("--limit".into()); cli.push(v.to_string()); }
    if let Some(v) = int_val(args, "max_content_chars") { cli.push("--max-content-chars".into()); cli.push(v.to_string()); }
    if let Some(v) = str_val(args, "sort") { cli.push("--sort".into()); cli.push(v); }
    if bool_val(args, "has_link") { cli.push("--has-link".into()); }
    if bool_val(args, "has_emoji") { cli.push("--has-emoji".into()); }
    if bool_val(args, "from_me") { cli.push("--from-me".into()); }
    if bool_val(args, "resolve_users") { cli.push("--resolve-users".into()); }
    if bool_val(args, "refresh_users") { cli.push("--refresh-users".into()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_count() {
        let tools = all_tools();
        assert!(tools.len() > 40);
    }

    #[test]
    fn test_tool_names_unique() {
        let tools = all_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), tools.len());
    }

    #[test]
    fn test_message_send_args() {
        let args = json!({
            "target": "#general",
            "text": "Hello world",
            "thread_ts": "123.456"
        });
        let cli = tool_to_cli_args("message_send", &args).unwrap();
        assert_eq!(cli, vec!["message", "send", "#general", "Hello world", "--thread-ts", "123.456"]);
    }

    #[test]
    fn test_search_messages_args() {
        let args = json!({
            "query": "test query",
            "limit": 10,
            "has_link": true
        });
        let cli = tool_to_cli_args("search_messages", &args).unwrap();
        assert!(cli.contains(&"search".to_string()));
        assert!(cli.contains(&"messages".to_string()));
        assert!(cli.contains(&"test query".to_string()));
        assert!(cli.contains(&"--limit".to_string()));
        assert!(cli.contains(&"--has-link".to_string()));
    }

    #[test]
    fn test_unknown_tool_returns_none() {
        let args = json!({});
        assert!(tool_to_cli_args("nonexistent_tool", &args).is_none());
    }

    #[test]
    fn test_write_tools_flagged() {
        let tools = all_tools();
        let write_names: Vec<&str> = tools.iter().filter(|t| t.is_write).map(|t| t.name).collect();
        assert!(write_names.contains(&"message_send"));
        assert!(write_names.contains(&"message_delete"));
        assert!(write_names.contains(&"dm_send"));
        assert!(write_names.contains(&"channel_rename"));
        assert!(!write_names.contains(&"message_get"));
        assert!(!write_names.contains(&"search_messages"));
    }
}
