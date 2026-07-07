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

    if let Err(e) = commands::dispatch(cli.command, cli.read_only).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
