use crate::cli::ConfigCommand;
use crate::error::{Result, SlackersError};

const DEFAULT_CONFIG: &str = r#"# slackers configuration
# See: docs/user-guide.md § Configuration

[history]
# Resume interrupted `message history` runs automatically (default: true)
# auto_resume = true

# Drop system messages by subtype
# exclude_subtypes = ["channel_join", "channel_leave", "channel_topic", "channel_purpose", "bot_message"]

# Drop messages from specific users by ID
# exclude_users = ["USLACKBOT"]

[store]
# Enable the local SQLite store (default: false)
# enabled = false

# Which channels to sync: public | public_private | all | selected
# sync_scope = "public"

# Keep full Slack JSON payloads in the messages table (default: false)
# store_raw_json = false

# Warn/prune when DB exceeds this size in MB (0 = unlimited)
# max_db_size_mb = 500

# Run garbage collection before each sync (default: true)
# auto_gc = true

[store.defaults]
# Default retention for new subscriptions in days (null = keep forever)
# retention_days = 90

# Sync full thread replies (default: true)
# sync_threads = true

# Sync channel membership lists (default: false)
# sync_members = false

# Sync file attachments (default: false)
# sync_files = false

# Max file size to download in MB (default: 10)
# max_file_size_mb = 10
"#;

pub async fn handle_config(cmd: ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Init => handle_config_init().await,
        ConfigCommand::Path => handle_config_path(),
    }
}

async fn handle_config_init() -> Result<()> {
    let path = crate::app_config::config_toml_path()
        .map_err(|_| SlackersError::Other("cannot determine config directory".into()))?;

    if path.exists() {
        eprintln!("Config file already exists at {}", path.display());
        eprintln!("To reset, delete it first and re-run `slackers config init`.");
        let info = serde_json::json!({
            "ok": false,
            "path": path.to_string_lossy(),
            "message": "config file already exists",
        });
        println!("{}", crate::output::to_json_output(&info));
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&path, DEFAULT_CONFIG)?;

    let info = serde_json::json!({
        "ok": true,
        "path": path.to_string_lossy(),
        "message": "config file created — edit to customize",
    });
    println!("{}", crate::output::to_json_output(&info));
    Ok(())
}

fn handle_config_path() -> Result<()> {
    let path = crate::app_config::config_toml_path()
        .map_err(|_| SlackersError::Other("cannot determine config directory".into()))?;

    let exists = path.exists();
    let info = serde_json::json!({
        "path": path.to_string_lossy(),
        "exists": exists,
    });
    println!("{}", crate::output::to_json_output(&info));
    Ok(())
}
