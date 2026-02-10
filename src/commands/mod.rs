mod auth;
mod canvas;
mod message;
mod search;
mod user;

use crate::cli::Command;
use crate::error::Result;

pub async fn dispatch(command: Command) -> Result<()> {
    match command {
        Command::Auth { subcommand } => {
            auth::handle_auth(subcommand).await
        }
        Command::Message { subcommand } => {
            message::handle_message(subcommand).await
        }
        Command::Search { subcommand } => {
            search::handle_search(subcommand).await
        }
        Command::Canvas { subcommand } => {
            canvas::handle_canvas(subcommand).await
        }
        Command::User { subcommand } => {
            user::handle_user(subcommand).await
        }
    }
}



