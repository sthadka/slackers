mod auth;
mod batch;
mod canvas;
mod channel;
pub mod dm;
pub mod export;
pub mod file;
mod message;
pub mod mention;
pub mod later;
pub mod scheduled;
mod search;
mod user;
pub mod workspace;

use crate::cli::{BatchCommand, Command, DmCommand, EmojiCommand, ExportCommand, LaterCommand, MentionCommand, WorkspaceCommand};
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
        Command::Later { subcommand } => match subcommand {
            LaterCommand::Add(opts) => later::handle_later_add(opts).await,
            LaterCommand::Remove(opts) => later::handle_later_remove(opts).await,
            LaterCommand::List(opts) => later::handle_later_list(opts).await,
        },
        Command::Scheduled { subcommand } => {
            scheduled::handle_scheduled(subcommand).await
        }
        Command::Dm { subcommand } => match subcommand {
            DmCommand::Open(opts) => dm::handle_dm_open(opts.workspace.as_deref(), opts.users).await,
            DmCommand::Send(opts) => dm::handle_dm_send(opts.workspace.as_deref(), opts.users, opts.message).await,
        },
        Command::Mention { subcommand } => match subcommand {
            MentionCommand::List(opts) => {
                mention::handle_mention_list(
                    opts.workspace.as_deref(),
                    opts.username,
                    opts.channel,
                    opts.after,
                    opts.before,
                    opts.limit,
                )
                .await
            }
        },
        Command::Export { subcommand } => match subcommand {
            ExportCommand::Channel(opts) => {
                export::run_export(
                    &opts.channel,
                    &opts.format,
                    opts.output.as_deref(),
                    opts.workspace.as_deref(),
                )
                .await
            }
        },
    }
}



