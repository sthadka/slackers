pub mod chrome;
pub mod curl;
pub mod desktop;
pub mod keychain;
pub mod resolver;
pub mod store;
pub mod types;

pub use chrome::extract_from_chrome;
pub use curl::parse_curl_from_stdin;
pub use desktop::extract_from_slack_desktop;
pub use resolver::resolve_auth;
pub use store::{
    load_credentials, remove_workspace, set_default_workspace, upsert_workspace, upsert_workspaces,
};
pub use types::{normalize_workspace_url, Workspace, WorkspaceAuth};
