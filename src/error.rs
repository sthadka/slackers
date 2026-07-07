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

    pub fn error_type(&self) -> &str {
        match self {
            SlackersError::SlackApi { .. } => "slack_api",
            SlackersError::Auth(_) => "auth",
            SlackersError::Http(_) => "network",
            SlackersError::Io(_) => "io",
            SlackersError::Json(_) | SlackersError::Parse(_) => "parse",
            SlackersError::Database(_) => "database",
            SlackersError::Config(_) | SlackersError::Other(_) => "other",
        }
    }

    pub fn error_code(&self) -> Option<&str> {
        match self {
            SlackersError::SlackApi { error_code, .. } => error_code.as_deref(),
            _ => None,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, SlackersError::Http(_))
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            SlackersError::Auth(_) => 3,
            SlackersError::Http(_) => 4,
            SlackersError::SlackApi { .. } => 5,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_type() {
        assert_eq!(
            SlackersError::from_slack_api("test", Some("channel_not_found".into())).error_type(),
            "slack_api"
        );
        assert_eq!(
            SlackersError::Auth(AuthError::NoCredentials).error_type(),
            "auth"
        );
        assert_eq!(
            SlackersError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "nope")).error_type(),
            "io"
        );
        assert_eq!(
            SlackersError::Parse(ParseError::InvalidUrl("x".into())).error_type(),
            "parse"
        );
        assert_eq!(
            SlackersError::Other("misc".into()).error_type(),
            "other"
        );
        assert_eq!(
            SlackersError::Config(ConfigError::DirectoryNotFound).error_type(),
            "other"
        );
        assert_eq!(
            SlackersError::Database(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                None,
            )).error_type(),
            "database"
        );
    }

    #[test]
    fn test_error_code() {
        let with_code = SlackersError::from_slack_api("test", Some("channel_not_found".into()));
        assert_eq!(with_code.error_code(), Some("channel_not_found"));

        let without_code = SlackersError::from_slack_api("test", None);
        assert_eq!(without_code.error_code(), None);

        let auth = SlackersError::Auth(AuthError::NoCredentials);
        assert_eq!(auth.error_code(), None);

        let other = SlackersError::Other("misc".into());
        assert_eq!(other.error_code(), None);
    }

    #[test]
    fn test_is_retryable() {
        assert!(!SlackersError::Auth(AuthError::NoCredentials).is_retryable());
        assert!(!SlackersError::from_slack_api("test", None).is_retryable());
        assert!(!SlackersError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "nope")).is_retryable());
        assert!(!SlackersError::Other("misc".into()).is_retryable());
    }

    #[test]
    fn test_exit_code() {
        assert_eq!(SlackersError::Auth(AuthError::NoCredentials).exit_code(), 3);
        assert_eq!(SlackersError::from_slack_api("test", None).exit_code(), 5);
        assert_eq!(
            SlackersError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "nope")).exit_code(),
            1
        );
        assert_eq!(SlackersError::Other("misc".into()).exit_code(), 1);
        assert_eq!(
            SlackersError::Parse(ParseError::InvalidUrl("x".into())).exit_code(),
            1
        );
        assert_eq!(
            SlackersError::Database(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                None,
            )).exit_code(),
            1
        );
    }
}
