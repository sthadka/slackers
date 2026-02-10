use thiserror::Error;

#[derive(Error, Debug)]
pub enum SlackersError {
    #[error("Slack API error: {message}")]
    SlackApi {
        message: String,
        error_code: Option<String>,
    },

    #[error("Authentication error: {0}")]
    Auth(#[from] AuthError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("{0}")]
    Other(String),
}

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum AuthError {
    #[error("No credentials found. Run 'slackers auth add' or set SLACK_TOKEN environment variable")]
    NoCredentials,

    #[error("Invalid authentication: {0}")]
    InvalidAuth(String),

    #[error("Token expired or revoked")]
    TokenExpired,

    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(String),

    #[error("Multiple workspaces configured but no default set. Use --workspace or 'slackers auth set-default'")]
    NoDefaultWorkspace,

    #[error("Keychain error: {0}")]
    Keychain(String),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(String),

    #[error("Failed to write config file: {0}")]
    WriteError(String),

    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Config directory not found or inaccessible")]
    DirectoryNotFound,
}

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum ParseError {
    #[error("Invalid Slack URL: {0}")]
    InvalidUrl(String),

    #[error("Invalid message timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Invalid channel ID or name: {0}")]
    InvalidChannel(String),

    #[error("Invalid user ID or handle: {0}")]
    InvalidUser(String),

    #[error("Invalid canvas URL or ID: {0}")]
    InvalidCanvas(String),

    #[error("Malformed target: {0}")]
    MalformedTarget(String),
}

// Result type alias for convenience
pub type Result<T> = std::result::Result<T, SlackersError>;

// Implement From<String> for SlackersError for easy error creation
impl From<String> for SlackersError {
    fn from(s: String) -> Self {
        SlackersError::Other(s)
    }
}

impl From<&str> for SlackersError {
    fn from(s: &str) -> Self {
        SlackersError::Other(s.to_string())
    }
}

// Helper to create SlackApiError from Slack API response
impl SlackersError {
    pub fn from_slack_api(error_msg: impl Into<String>, code: Option<String>) -> Self {
        SlackersError::SlackApi {
            message: error_msg.into(),
            error_code: code,
        }
    }

    /// Check if this is an auth-related error that might benefit from token refresh
    #[allow(dead_code)]
    pub fn is_auth_error(&self) -> bool {
        match self {
            SlackersError::Auth(AuthError::InvalidAuth(_))
            | SlackersError::Auth(AuthError::TokenExpired) => true,
            SlackersError::SlackApi {
                error_code: Some(code),
                ..
            } => code == "invalid_auth" || code == "token_expired" || code == "token_revoked",
            _ => false,
        }
    }
}
