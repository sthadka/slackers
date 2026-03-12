use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "slackers")]
#[command(about = "Rust clone of agent-slack - Slack automation CLI for AI agents")]
#[command(version)]
pub struct Cli {
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

        /// Message text to post
        text: String,

        /// Workspace URL (needed when using #channel/channel id and you have multiple workspaces)
        #[arg(long)]
        workspace: Option<String>,

        /// Thread root ts to post into (optional)
        #[arg(long)]
        thread_ts: Option<String>,
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
    #[arg(long, default_value = "8000")]
    pub max_body_chars: i32,

    /// Fetch and inline full thread replies for threaded messages
    #[arg(long)]
    pub include_threads: bool,

    /// Include reactions on messages
    #[arg(long)]
    pub include_reactions: bool,
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
    #[arg(long, default_value = "8000")]
    pub max_body_chars: i32,

    /// Include reactions + reacting users
    #[arg(long)]
    pub include_reactions: bool,
}

#[derive(Args, Debug)]
pub struct MessageListOptions {
    /// Workspace URL (needed when using #channel/channel id and you have multiple workspaces)
    #[arg(long)]
    pub workspace: Option<String>,

    /// Thread root ts (required when using #channel/channel id unless you pass --ts)
    #[arg(long)]
    pub thread_ts: Option<String>,

    /// Message ts (optional: resolve message to its thread)
    #[arg(long)]
    pub ts: Option<String>,

    /// Max content characters to include (default 8000, -1 for unlimited)
    #[arg(long, default_value = "8000")]
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

    /// Max message content characters (default 4000, -1 for unlimited)
    #[arg(long, default_value = "4000")]
    pub max_content_chars: i32,
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
        max_chars: i32,
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
}
