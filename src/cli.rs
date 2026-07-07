use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "slackers")]
#[command(about = "Rust clone of agent-slack - Slack automation CLI for AI agents")]
#[command(version)]
pub struct Cli {
    /// Block all write operations (send, update, delete, pin, react, etc.)
    #[arg(long, global = true)]
    pub read_only: bool,

    /// Produce indented JSON instead of compact single-line JSON
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Minimal JSON output for write operations (e.g. {"ok":true})
    #[arg(long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Manage Slack authentication
    Auth {
        #[command(subcommand)]
        subcommand: AuthCommand,
    },
    /// Read/write Slack messages (token-efficient JSON)
    Message {
        #[command(subcommand)]
        subcommand: MessageCommand,
    },
    /// Search Slack messages and files (token-efficient JSON)
    Search {
        #[command(subcommand)]
        subcommand: SearchCommand,
    },
    /// Work with Slack canvases
    Canvas {
        #[command(subcommand)]
        subcommand: CanvasCommand,
    },
    /// Workspace user directory
    User {
        #[command(subcommand)]
        subcommand: UserCommand,
    },
    /// Channel discovery and management
    Channel {
        #[command(subcommand)]
        subcommand: ChannelCommand,
    },
    /// Batch operations (send messages or add reactions to multiple targets)
    Batch {
        #[command(subcommand)]
        subcommand: BatchCommand,
    },
    /// Upload, delete, or list Slack files
    File {
        #[command(subcommand)]
        subcommand: FileCommand,
    },
    /// Workspace information (team name, domain, icon)
    Workspace {
        #[command(subcommand)]
        subcommand: WorkspaceCommand,
    },
    /// Emoji directory for the workspace
    Emoji {
        #[command(subcommand)]
        subcommand: EmojiCommand,
    },
    /// Save messages for later (stars) and list saved items
    Later {
        #[command(subcommand)]
        subcommand: LaterCommand,
    },
    /// Schedule, list, and delete scheduled messages
    Scheduled {
        #[command(subcommand)]
        subcommand: ScheduledCommand,
    },
    /// Open and send direct messages
    Dm {
        #[command(subcommand)]
        subcommand: DmCommand,
    },
    /// List messages that @mention you or a user
    Mention {
        #[command(subcommand)]
        subcommand: MentionCommand,
    },
    /// Export channel history in various formats
    Export {
        #[command(subcommand)]
        subcommand: ExportCommand,
    },
    /// Show unread messages across channels, DMs, and threads
    Unreads {
        #[command(subcommand)]
        subcommand: UnreadsCommand,
    },
    /// Discover and interact with Slack workflows
    Workflow {
        #[command(subcommand)]
        subcommand: WorkflowCommand,
    },
    /// Execute slash commands in channels (requires browser token)
    Slash {
        #[command(subcommand)]
        subcommand: SlashCommand,
    },
    /// Start an MCP (Model Context Protocol) server over stdio
    Serve,
}

// ============================================================================
// Slash Command Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum SlashCommand {
    /// Execute a slash command in a channel
    Run(SlashRunOptions),
}

#[derive(Args, Debug)]
pub struct SlashRunOptions {
    /// Channel ID (C...) or #name/name where the command will run
    #[arg(long)]
    pub channel: String,

    /// Slash command with arguments (e.g. "/remind me to check in 1 hour")
    pub command: Vec<String>,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// File Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum FileCommand {
    /// Upload a file to Slack
    Upload(FileUploadOptions),

    /// Delete a file by ID
    Delete(FileDeleteOptions),

    /// List files in the workspace or a channel
    List(FileListOptions),
}

#[derive(Args, Debug)]
pub struct FileUploadOptions {
    /// Local path of the file to upload
    #[arg(long)]
    pub file: String,

    /// Comma-separated list of channel IDs or names to share the file into
    #[arg(long, value_delimiter = ',')]
    pub channels: Option<Vec<String>>,

    /// Initial comment to accompany the file
    #[arg(long)]
    pub comment: Option<String>,

    /// Display title for the file in Slack
    #[arg(long)]
    pub title: Option<String>,

    /// Override the on-disk filename shown in Slack
    #[arg(long)]
    pub filename: Option<String>,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct FileDeleteOptions {
    /// Slack file ID (F...)
    #[arg(long)]
    pub file_id: String,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct FileListOptions {
    /// Filter by channel ID (C...)
    #[arg(long)]
    pub channel: Option<String>,

    /// Max number of files to return (default 100)
    #[arg(long, default_value = "100")]
    pub limit: u32,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Batch Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum BatchCommand {
    /// Send a message to multiple channels
    Send(BatchSendOptions),

    /// Add a reaction to multiple messages
    React(BatchReactOptions),
}

#[derive(Args, Debug)]
pub struct BatchSendOptions {
    /// Message text to send
    #[arg(long)]
    pub message: String,

    /// Comma-separated list of channels (e.g. #general,#random or C123,C456)
    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct BatchReactOptions {
    /// Emoji name to react with (:rocket:, rocket, or 🚀)
    #[arg(long)]
    pub emoji: String,

    /// Comma-separated list of Slack message URLs
    #[arg(long, value_delimiter = ',')]
    pub messages: Vec<String>,
}

// ============================================================================
// Auth Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Show configured workspaces and token sources
    Whoami,

    /// Verify credentials (calls Slack auth.test)
    Test {
        /// Workspace URL (needed when you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Add credentials (standard token or browser xoxc/xoxd)
    Add {
        /// Workspace URL like https://myteam.slack.com
        #[arg(long)]
        workspace_url: String,

        /// Standard Slack token (xoxb/xoxp)
        #[arg(long)]
        token: Option<String>,

        /// Browser token (xoxc-...)
        #[arg(long)]
        xoxc: Option<String>,

        /// Browser cookie d (xoxd-...)
        #[arg(long)]
        xoxd: Option<String>,
    },

    /// Set the default workspace URL
    SetDefault {
        /// Workspace URL like https://myteam.slack.com
        workspace_url: String,
    },

    /// Remove a workspace from local config
    Remove {
        /// Workspace URL like https://myteam.slack.com
        workspace_url: String,
    },

    /// Import xoxc token(s) + d cookie from Slack Desktop data (no need to quit Slack)
    ImportDesktop,

    /// Import xoxc/xoxd from a logged-in Slack tab in Google Chrome (macOS)
    ImportChrome,

    /// Paste a Slack API request copied as cURL (extracts xoxc/xoxd and saves locally)
    ParseCurl,
}

// ============================================================================
// Message Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum MessageCommand {
    /// Fetch a single Slack message (with thread summary if any)
    Get {
        /// Slack message URL, #channel, or channel ID
        target: String,

        #[command(flatten)]
        options: MessageGetOptions,
    },

    /// Fetch the full thread for a Slack message URL
    List {
        /// Slack message URL, #channel, or channel ID
        target: String,

        #[command(flatten)]
        options: MessageListOptions,
    },

    /// Send a message (optionally into a thread)
    Send {
        /// Slack message URL, #name/name, or channel id
        target: String,

        /// Message text to post (used as notification fallback when --blocks is provided)
        text: String,

        /// Workspace URL (needed when using #channel/channel id and you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Thread root ts to post into (optional)
        #[arg(long)]
        thread_ts: Option<String>,

        /// Also broadcast the threaded reply to the channel (reply_broadcast)
        #[arg(long)]
        reply_broadcast: bool,

        /// Path to a JSON file containing a Block Kit blocks array (use - for stdin)
        #[arg(long)]
        blocks: Option<String>,
    },

    /// Add or remove reactions
    React {
        #[command(subcommand)]
        subcommand: ReactCommand,
    },

    /// Fetch all messages from a channel, with optional thread expansion
    History {
        /// Channel name (#name) or ID (C...)
        channel: String,

        #[command(flatten)]
        options: MessageHistoryOptions,
    },

    /// List unique participants in a thread with message counts
    ThreadParticipants {
        /// Slack thread URL, or omit and use --channel + --ts
        target: Option<String>,

        /// Channel ID (required when not using a URL target)
        #[arg(long)]
        channel: Option<String>,

        /// Thread root ts (required when not using a URL target)
        #[arg(long)]
        ts: Option<String>,

        /// Workspace URL (needed when you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Resolve user IDs to display names via users.info
        #[arg(long)]
        resolve_users: bool,
    },

    /// Pin a message to a channel
    Pin(MessagePinOptions),

    /// Unpin a message from a channel
    Unpin(MessageUnpinOptions),

    /// Delete a message
    Delete(MessageDeleteOptions),

    /// Update the text of an existing message
    Update(MessageUpdateOptions),
}

#[derive(Args, Debug)]
pub struct MessagePinOptions {
    /// Channel ID (C...) that contains the message
    #[arg(long)]
    pub channel: String,

    /// Message timestamp (unique message ID)
    #[arg(long)]
    pub ts: String,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct MessageUnpinOptions {
    /// Channel ID (C...) that contains the message
    #[arg(long)]
    pub channel: String,

    /// Message timestamp (unique message ID)
    #[arg(long)]
    pub ts: String,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct MessageDeleteOptions {
    /// Channel ID (C...) that contains the message
    #[arg(long)]
    pub channel: String,

    /// Message timestamp (unique message ID)
    #[arg(long)]
    pub ts: String,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct MessageUpdateOptions {
    /// Channel ID (C...) that contains the message
    #[arg(long)]
    pub channel: String,

    /// Message timestamp (unique message ID)
    #[arg(long)]
    pub ts: String,

    /// New text for the message
    #[arg(long)]
    pub text: String,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct MessageHistoryOptions {
    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,

    /// Max top-level messages to fetch
    #[arg(long, default_value = "500")]
    pub limit: usize,

    /// Only messages after YYYY-MM-DD
    #[arg(long)]
    pub after: Option<String>,

    /// Only messages before YYYY-MM-DD
    #[arg(long)]
    pub before: Option<String>,

    /// Max message body characters (-1 for unlimited, default 8000)
    #[arg(long, default_value = "8000", allow_negative_numbers = true)]
    pub max_body_chars: i32,

    /// Fetch and inline full thread replies for threaded messages
    #[arg(long)]
    pub include_threads: bool,

    /// Include reactions on messages
    #[arg(long)]
    pub include_reactions: bool,

    /// Output file path. Defaults to <channel>-history.json in the current directory.
    /// Messages are written incrementally after each page so the file is always
    /// up to date. An existing file is used as the starting point for resume.
    #[arg(long, short = 'o')]
    pub output: Option<String>,
}

#[derive(Args, Debug)]
pub struct MessageGetOptions {
    /// Workspace URL (needed when using #channel/channel id and you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,

    /// Message ts (required when using #channel/channel id)
    #[arg(long)]
    pub ts: Option<String>,

    /// Thread root ts hint (useful for thread permalinks)
    #[arg(long)]
    pub thread_ts: Option<String>,

    /// Max content characters to include (default 8000, -1 for unlimited)
    #[arg(long, default_value = "8000", allow_negative_numbers = true)]
    pub max_body_chars: i32,

    /// Include reactions + reacting users
    #[arg(long)]
    pub include_reactions: bool,

    /// Resolve user IDs to display names via users.info
    #[arg(long)]
    pub resolve_users: bool,

    /// Force refresh user cache (ignore 24h TTL, re-fetch from API)
    #[arg(long)]
    pub refresh_users: bool,
}

#[derive(Args, Debug)]
pub struct MessageListOptions {
    /// Workspace URL (needed when using #channel/channel id and you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,

    /// Output format: json (default), table, markdown, plain
    #[arg(long, default_value = "json")]
    pub format: String,

    /// Thread root ts (required when using #channel/channel id unless you pass --ts)
    #[arg(long)]
    pub thread_ts: Option<String>,

    /// Message ts (optional: resolve message to its thread)
    #[arg(long)]
    pub ts: Option<String>,

    /// Max content characters to include (default 8000, -1 for unlimited)
    #[arg(long, default_value = "8000", allow_negative_numbers = true)]
    pub max_body_chars: i32,

    /// Include reactions + reacting users
    #[arg(long)]
    pub include_reactions: bool,

    /// Maximum number of messages to return (default: 100)
    #[arg(long)]
    pub limit: Option<usize>,

    /// Only messages after this timestamp (format: seconds.micros or YYYY-MM-DD)
    #[arg(long)]
    pub after_ts: Option<String>,

    /// Only messages before this timestamp (format: seconds.micros or YYYY-MM-DD)
    #[arg(long)]
    pub before_ts: Option<String>,

    /// Filter by user ID (U...) or @handle
    #[arg(long)]
    pub user: Option<String>,

    /// Only show messages with links
    #[arg(long)]
    pub has_link: bool,

    /// Only show messages with file attachments
    #[arg(long)]
    pub has_file: bool,

    /// Only show messages with reactions
    #[arg(long)]
    pub has_reaction: bool,

    /// Only show messages that have this reaction (emoji name, e.g. thumbsup)
    #[arg(long)]
    pub with_reaction: Option<String>,

    /// Only show messages that do NOT have this reaction (emoji name, e.g. thumbsup)
    #[arg(long)]
    pub without_reaction: Option<String>,

    /// Resolve user IDs to display names via users.info
    #[arg(long)]
    pub resolve_users: bool,

    /// Force refresh user cache (ignore 24h TTL, re-fetch from API)
    #[arg(long)]
    pub refresh_users: bool,
}

#[derive(Subcommand, Debug)]
pub enum ReactCommand {
    /// Add a reaction to a message
    Add {
        /// Slack message URL, #channel, or channel ID
        target: String,

        /// Emoji to react with (:rocket:, rocket, or 🚀)
        emoji: String,

        /// Workspace URL (needed when using #channel/channel id and you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Message ts (required when using #channel/channel id)
        #[arg(long)]
        ts: Option<String>,
    },

    /// Remove a reaction from a message
    Remove {
        /// Slack message URL, #channel, or channel ID
        target: String,

        /// Emoji to remove (:rocket:, rocket, or 🚀)
        emoji: String,

        /// Workspace URL (needed when using #channel/channel id and you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Message ts (required when using #channel/channel id)
        #[arg(long)]
        ts: Option<String>,
    },
}

// ============================================================================
// Search Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum SearchCommand {
    /// Search messages and files
    All {
        /// Search query
        query: String,

        #[command(flatten)]
        options: SearchOptions,
    },

    /// Search messages
    Messages {
        /// Search query
        query: String,

        #[command(flatten)]
        options: SearchOptions,
    },

    /// Search files
    Files {
        /// Search query
        query: String,

        #[command(flatten)]
        options: SearchOptions,
    },
}

#[derive(Args, Debug)]
pub struct SearchOptions {
    /// Workspace URL (needed when searching across multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,

    /// Output format: json (default), table, markdown, plain
    #[arg(long, default_value = "json")]
    pub format: String,

    /// Channel filter (#name, name, or id). Repeatable.
    #[arg(long)]
    pub channel: Vec<String>,

    /// User filter (@name, name, or user id U...)
    #[arg(long)]
    pub user: Option<String>,

    /// Only results after YYYY-MM-DD
    #[arg(long)]
    pub after: Option<String>,

    /// Only results before YYYY-MM-DD
    #[arg(long)]
    pub before: Option<String>,

    /// Filter content type: any|text|image|snippet|file (default any)
    #[arg(long)]
    pub content_type: Option<String>,

    /// Max results (default 20)
    #[arg(long, default_value = "20")]
    pub limit: u32,

    /// Max message body characters (default 4000, -1 for unlimited)
    #[arg(long, default_value = "4000")]
    pub max_body_chars: i32,

    /// Sort order: timestamp (default) or relevance
    #[arg(long)]
    pub sort: Option<String>,

    /// Only results containing a URL (has:link)
    #[arg(long, default_value = "false")]
    pub has_link: bool,

    /// Only results containing an emoji reaction (has:emoji)
    #[arg(long, default_value = "false")]
    pub has_emoji: bool,

    /// Only results sent by the authenticated user (from:me)
    #[arg(long, default_value = "false")]
    pub from_me: bool,

    /// Resolve user IDs to display names via users.info
    #[arg(long)]
    pub resolve_users: bool,

    /// Force refresh user cache (ignore 24h TTL, re-fetch from API)
    #[arg(long)]
    pub refresh_users: bool,
}

// ============================================================================
// Canvas Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum CanvasCommand {
    /// Fetch a Slack canvas and convert it to Markdown
    Get {
        /// Slack canvas URL (…/docs/…/F…) or canvas id (F…)
        canvas: String,

        /// Workspace URL (required if passing a canvas id and you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Max markdown characters to include (default 20000, -1 for unlimited)
        #[arg(long, default_value = "20000")]
        max_body_chars: i32,
    },
}

// ============================================================================
// User Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum UserCommand {
    /// List users in the workspace
    List {
        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Max users (default 200)
        #[arg(long, default_value = "200")]
        limit: u32,

        /// Pagination cursor
        #[arg(long)]
        cursor: Option<String>,

        /// Include bot users
        #[arg(long)]
        include_bots: bool,

        /// Output format: json (default), table, markdown, plain
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Get a single user by id (U...) or handle (@name)
    Get {
        /// User id (U...) or @handle/handle
        user: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },
}

// ============================================================================
// Channel Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum ChannelCommand {
    /// List channels/conversations in the workspace
    List {
        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Conversation types (public_channel, private_channel, mpim, im). Repeatable.
        #[arg(long)]
        types: Option<Vec<String>>,

        /// Exclude archived channels
        #[arg(long, default_value = "true")]
        exclude_archived: bool,

        /// Max channels (default 200)
        #[arg(long, default_value = "200")]
        limit: u32,

        /// Resolve user IDs to display names for DMs (im type)
        #[arg(long)]
        resolve_users: bool,

        /// Show all channels, not just ones you've joined
        #[arg(long)]
        all: bool,

        /// Output format: json (default), table, markdown, plain
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Get detailed information about a channel
    Get {
        /// Channel id (C...) or #name/name
        channel: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Include member count
        #[arg(long)]
        include_num_members: bool,
    },

    /// Join a channel
    Join {
        /// Channel id (C...) or #name/name
        channel: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Leave a channel
    Leave {
        /// Channel id (C...) or #name/name
        channel: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Mark a channel or DM as read up to a given message timestamp
    Mark(ChannelMarkOptions),

    /// List members of a channel
    Members(ChannelMembersOptions),

    /// Create a new channel
    New(ChannelNewOptions),

    /// Invite users to a channel
    Invite(ChannelInviteOptions),

    /// Rename a channel
    Rename {
        /// Channel id (C...) or #name/name
        channel: String,

        /// New name for the channel
        name: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ChannelMembersOptions {
    /// Channel id (C...) or #name/name
    pub target: String,

    /// Resolve user IDs to display names via users.info
    #[arg(long)]
    pub resolve_users: bool,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct ChannelNewOptions {
    /// Channel name (lowercase letters, numbers, hyphens, underscores)
    #[arg(long)]
    pub name: String,

    /// Create as a private channel
    #[arg(long)]
    pub private: bool,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct ChannelInviteOptions {
    /// Channel id (C...) or #name/name
    pub target: String,

    /// Comma-separated list of user IDs to invite
    #[arg(long, value_delimiter = ',')]
    pub users: Vec<String>,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct ChannelMarkOptions {
    /// Channel id (C...), DM id (D...), or #name/name
    pub target: String,

    /// Message timestamp to mark as read up to
    #[arg(long)]
    pub ts: String,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Workspace Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum WorkspaceCommand {
    /// Fetch workspace information (id, name, domain, icon)
    Info(WorkspaceInfoOptions),
}

#[derive(Args, Debug)]
pub struct WorkspaceInfoOptions {
    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Emoji Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum EmojiCommand {
    /// List all custom emoji for the workspace
    List(EmojiListOptions),
}

#[derive(Args, Debug)]
pub struct EmojiListOptions {
    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Later Commands (starred / saved-for-later messages)
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum LaterCommand {
    /// Star (save for later) a message
    Add(LaterAddOptions),

    /// Unstar (remove from saved) a message
    Remove(LaterRemoveOptions),

    /// List starred (saved for later) items
    List(LaterListOptions),
}

#[derive(Args, Debug)]
pub struct LaterAddOptions {
    /// Channel ID containing the message
    #[arg(long)]
    pub channel: String,

    /// Message timestamp
    #[arg(long)]
    pub ts: String,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct LaterRemoveOptions {
    /// Channel ID containing the message
    #[arg(long)]
    pub channel: String,

    /// Message timestamp
    #[arg(long)]
    pub ts: String,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct LaterListOptions {
    /// Maximum number of starred items to return (default 100)
    #[arg(long, default_value = "100")]
    pub limit: usize,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Scheduled Message Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum ScheduledCommand {
    /// Schedule a message to be sent at a future time
    Send(MessageScheduleOptions),

    /// List scheduled messages (optionally filtered by channel)
    List(MessageScheduledListOptions),

    /// Delete a scheduled message
    Delete(MessageScheduledDeleteOptions),
}

#[derive(Args, Debug)]
pub struct MessageScheduleOptions {
    /// Channel ID or name to send the message to
    #[arg(long)]
    pub channel: String,

    /// Message text to schedule
    #[arg(long)]
    pub message: String,

    /// Unix timestamp or RFC3339 datetime when to post the message
    #[arg(long)]
    pub at: String,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct MessageScheduledListOptions {
    /// Filter by channel ID or name (optional)
    #[arg(long)]
    pub channel: Option<String>,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct MessageScheduledDeleteOptions {
    /// Channel ID containing the scheduled message
    #[arg(long)]
    pub channel: String,

    /// Scheduled message ID to delete
    #[arg(long)]
    pub id: String,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// DM Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum DmCommand {
    /// Open a direct message conversation with one or more users
    Open(DmOpenOptions),

    /// Open a DM and send a message
    Send(DmSendOptions),
}

#[derive(Args, Debug)]
pub struct DmOpenOptions {
    /// Comma-separated list of user IDs to open a DM with
    #[arg(long, value_delimiter = ',')]
    pub users: Vec<String>,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

#[derive(Args, Debug)]
pub struct DmSendOptions {
    /// Comma-separated list of user IDs to open a DM with
    #[arg(long, value_delimiter = ',')]
    pub users: Vec<String>,

    /// Message text to send
    #[arg(long)]
    pub message: String,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Mention Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum MentionCommand {
    /// List messages that @mention you or a named user
    List(MentionListOptions),
}

#[derive(Args, Debug)]
pub struct MentionListOptions {
    /// Username to search for mentions of (defaults to authenticated user)
    #[arg(long)]
    pub username: Option<String>,

    /// Channel filter (#name, name, or id). Repeatable.
    #[arg(long)]
    pub channel: Vec<String>,

    /// Only results after YYYY-MM-DD
    #[arg(long)]
    pub after: Option<String>,

    /// Only results before YYYY-MM-DD
    #[arg(long)]
    pub before: Option<String>,

    /// Max results (default 20)
    #[arg(long, default_value = "20")]
    pub limit: usize,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,

    /// Max message body characters (default 4000, -1 for unlimited)
    #[arg(long, default_value = "4000")]
    pub max_body_chars: i32,
}

// ============================================================================
// Export Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum ExportCommand {
    /// Export channel history
    Channel(ExportChannelOptions),
}

#[derive(Args, Debug)]
pub struct ExportChannelOptions {
    /// Channel id (C...) or #name/name
    #[arg(long)]
    pub channel: String,

    /// Output format: json, csv, or html (default json)
    #[arg(long, default_value = "json")]
    pub format: String,

    /// Output file path (defaults to stdout)
    #[arg(long)]
    pub output: Option<String>,

    /// Workspace URL (needed when you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Unreads Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum UnreadsCommand {
    /// Show unread messages across all conversations
    Show(UnreadsShowOptions),
}

#[derive(Args, Debug)]
pub struct UnreadsShowOptions {
    /// Only show unread counts, do not fetch message content
    #[arg(long)]
    pub counts_only: bool,

    /// Max unread messages to fetch per channel (default 10)
    #[arg(long, default_value = "10")]
    pub max_messages: usize,

    /// Max content characters per message (default 4000, -1 for unlimited)
    #[arg(long, default_value = "4000", allow_hyphen_values = true)]
    pub max_body_chars: i64,

    /// Include system messages (joins, leaves, topic changes, etc.)
    #[arg(long)]
    pub include_system: bool,

    /// Output format: json, table, markdown, plain
    #[arg(long)]
    pub format: Option<String>,

    /// Workspace URL (required if you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,
}

// ============================================================================
// Workflow Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum WorkflowCommand {
    /// List workflows bookmarked or featured in a channel
    List {
        /// Channel id or name (#channel, channel, C...)
        channel: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Get workflow metadata from a trigger ID (no side effects)
    Preview {
        /// Trigger ID (Ft...)
        trigger_id: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Get workflow definition including form fields and steps (accepts Ft... or Wf...)
    Get {
        /// Trigger ID (Ft...) or Workflow ID (Wf...)
        id: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Trip a workflow trigger
    Run {
        /// Trigger ID (Ft...)
        trigger_id: String,

        /// Channel where the workflow is bookmarked
        #[arg(long)]
        channel: String,

        /// Workspace URL (required if you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,
    },
}
