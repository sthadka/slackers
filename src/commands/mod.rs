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
mod slash;
mod unreads;
mod user;
mod workflow;
pub mod workspace;

use crate::cli::{
    BatchCommand, ChannelCommand, Command, DmCommand, EmojiCommand, ExportCommand,
    FileCommand, LaterCommand, MentionCommand, MessageCommand, ReactCommand,
    ScheduledCommand, SlashCommand, WorkflowCommand, WorkspaceCommand,
};
use crate::error::{Result, SlackersError};

fn check_read_only(read_only: bool) -> Result<()> {
    if read_only {
        Err(SlackersError::Other(
            "Operation blocked: --read-only mode is enabled".to_string(),
        ))
    } else {
        Ok(())
    }
}

pub async fn dispatch(command: Command, read_only: bool) -> Result<()> {
    match command {
        Command::Auth { subcommand } => {
            auth::handle_auth(subcommand).await
        }
        Command::Message { subcommand } => match subcommand {
            MessageCommand::Get { .. }
            | MessageCommand::List { .. }
            | MessageCommand::History { .. }
            | MessageCommand::ThreadParticipants { .. } => {
                message::handle_message(subcommand).await
            }
            MessageCommand::Send { .. }
            | MessageCommand::Update(..)
            | MessageCommand::Delete(..)
            | MessageCommand::Pin(..)
            | MessageCommand::Unpin(..) => {
                check_read_only(read_only)?;
                message::handle_message(subcommand).await
            }
            MessageCommand::React { subcommand: ref react_cmd } => {
                match react_cmd {
                    ReactCommand::Add { .. } | ReactCommand::Remove { .. } => {
                        check_read_only(read_only)?;
                    }
                }
                message::handle_message(subcommand).await
            }
        },
        Command::Search { subcommand } => {
            search::handle_search(subcommand).await
        }
        Command::Canvas { subcommand } => {
            canvas::handle_canvas(subcommand).await
        }
        Command::User { subcommand } => {
            user::handle_user(subcommand).await
        }
        Command::Channel { subcommand } => match subcommand {
            ChannelCommand::List { .. }
            | ChannelCommand::Get { .. }
            | ChannelCommand::Members(..)
            | ChannelCommand::Rename { .. } => {
                channel::handle_channel(subcommand).await
            }
            ChannelCommand::New(..)
            | ChannelCommand::Join { .. }
            | ChannelCommand::Leave { .. }
            | ChannelCommand::Invite(..)
            | ChannelCommand::Mark(..) => {
                check_read_only(read_only)?;
                channel::handle_channel(subcommand).await
            }
        },
        Command::Batch { subcommand } => {
            check_read_only(read_only)?;
            match subcommand {
                BatchCommand::Send(options) => batch::handle_batch_send(options).await,
                BatchCommand::React(options) => batch::handle_batch_react(options).await,
            }
        }
        Command::File { subcommand } => match subcommand {
            FileCommand::List(..) => {
                file::handle_file(subcommand).await
            }
            FileCommand::Upload(..) | FileCommand::Delete(..) => {
                check_read_only(read_only)?;
                file::handle_file(subcommand).await
            }
        },
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
            LaterCommand::List(opts) => later::handle_later_list(opts).await,
            LaterCommand::Add(opts) => {
                check_read_only(read_only)?;
                later::handle_later_add(opts).await
            }
            LaterCommand::Remove(opts) => {
                check_read_only(read_only)?;
                later::handle_later_remove(opts).await
            }
        },
        Command::Scheduled { subcommand } => match subcommand {
            ScheduledCommand::List(..) => {
                scheduled::handle_scheduled(subcommand).await
            }
            ScheduledCommand::Send(..) | ScheduledCommand::Delete(..) => {
                check_read_only(read_only)?;
                scheduled::handle_scheduled(subcommand).await
            }
        },
        Command::Dm { subcommand } => match subcommand {
            DmCommand::Open(opts) => dm::handle_dm_open(opts.workspace.as_deref(), opts.users).await,
            DmCommand::Send(opts) => {
                check_read_only(read_only)?;
                dm::handle_dm_send(opts.workspace.as_deref(), opts.users, opts.message).await
            }
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
        Command::Unreads { subcommand } => {
            unreads::handle_unreads(subcommand).await
        }
        Command::Workflow { subcommand } => match subcommand {
            WorkflowCommand::List { .. }
            | WorkflowCommand::Preview { .. }
            | WorkflowCommand::Get { .. } => {
                workflow::handle_workflow(subcommand).await
            }
            WorkflowCommand::Run { .. } => {
                check_read_only(read_only)?;
                workflow::handle_workflow(subcommand).await
            }
        },
        Command::Slash { subcommand } => match subcommand {
            SlashCommand::Run(..) => {
                check_read_only(read_only)?;
                slash::handle_slash(subcommand).await
            }
        },
        Command::Serve => {
            crate::mcp::run_server(read_only).await
        }
    }
}
