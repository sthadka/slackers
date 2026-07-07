mod app_config;
mod auth;
mod cli;
mod commands;
mod config;
mod error;
mod mcp;
mod output;
mod render;
mod slack;
mod target;
mod util;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    output::set_pretty(cli.pretty);

    if let Err(e) = commands::dispatch(cli.command, cli.read_only).await {
        let mut err_obj = serde_json::json!({
            "error": true,
            "type": e.error_type(),
            "message": e.to_string(),
            "retryable": e.is_retryable(),
        });
        if let Some(code) = e.error_code() {
            err_obj["code"] = serde_json::json!(code);
        }
        println!("{}", crate::output::to_json_output(&err_obj));
        eprintln!("Error: {}", e);
        std::process::exit(e.exit_code());
    }
}
