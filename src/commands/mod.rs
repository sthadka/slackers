mod auth;
mod batch;
mod canvas;
mod channel;
pub mod dm;
pub mod export;
pub mod file;
mod message;
pub mod mention;
mod search;
mod user;
pub mod workspace;

use crate::cli::{BatchCommand, Command, EmojiCommand, WorkspaceCommand};
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
        Command::Channel { subcommand } => {
            channel::handle_channel(subcommand).await
        }
        Command::Batch { subcommand } => match subcommand {
            BatchCommand::Send(options) => batch::handle_batch_send(options).await,
            BatchCommand::React(options) => batch::handle_batch_react(options).await,
        },
        Command::File { subcommand } => {
            file::handle_file(subcommand).await
        }
        Command::Workspace { subcommand } => match subcommand {
            WorkspaceCommand::Info(opts) => {
                workspace::handle_workspace_info(opts.workspace.as_deref()).await
            }
        },
        Command::Emoji { subcommand } => match subcommand {
            EmojiCommand::List(opts) => {
                workspace::handle_emoji_list(opts.workspace.as_deref()).await
            }
        },
    }
}



