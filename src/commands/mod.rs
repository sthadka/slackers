mod auth;
mod batch;
mod canvas;
mod channel;
mod config;
pub mod dm;
pub mod export;
pub mod file;
mod message;
pub mod mention;
pub mod later;
mod query;
pub mod scheduled;
mod report;
mod search;
mod slash;
mod store;
mod sync_cmd;
mod unreads;
mod user;
mod watch;
mod workflow;
pub mod workspace;

use crate::cli::{
    BatchCommand, ChannelCommand, Command, DmCommand, EmojiCommand, ExportCommand,
    FileCommand, LaterCommand, MentionCommand, MentionListOptions, MessageCommand,
    ReactCommand, ScheduledCommand, SlashCommand, UnreadsCommand, UnreadsShowOptions,
    WorkflowCommand, WorkspaceCommand, WorkspaceInfoOptions, EmojiListOptions,
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
            | MessageCommand::Thread { .. }
            | MessageCommand::List { .. }
            | MessageCommand::Participants { .. } => {
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
            | ChannelCommand::Cache { .. } => {
                channel::handle_channel(subcommand).await
            }
            ChannelCommand::Create(..)
            | ChannelCommand::Join { .. }
            | ChannelCommand::Leave { .. }
            | ChannelCommand::Invite(..)
            | ChannelCommand::Mark(..)
            | ChannelCommand::Rename { .. } => {
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
        Command::Workspace { subcommand } => {
            let sub = subcommand.unwrap_or(WorkspaceCommand::Info(WorkspaceInfoOptions {
                workspace: None,
            }));
            match sub {
                WorkspaceCommand::Info(opts) => {
                    workspace::handle_workspace_info(opts.workspace.as_deref()).await
                }
            }
        }
        Command::Emoji { subcommand } => {
            let sub = subcommand.unwrap_or(EmojiCommand::List(EmojiListOptions {
                workspace: None,
            }));
            match sub {
                EmojiCommand::List(opts) => {
                    workspace::handle_emoji_list(opts.workspace.as_deref()).await
                }
            }
        }
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
        Command::Mention { subcommand } => {
            let sub = subcommand.unwrap_or(MentionCommand::List(MentionListOptions {
                username: None,
                channel: Vec::new(),
                after: None,
                before: None,
                limit: 20,
                workspace: None,
                max_body_chars: 4000,
            }));
            match sub {
                MentionCommand::List(opts) => {
                    mention::handle_mention_list(
                        opts.workspace.as_deref(),
                        opts.username,
                        opts.channel,
                        opts.after,
                        opts.before,
                        opts.limit,
                        opts.max_body_chars,
                    )
                    .await
                }
            }
        }
        Command::Export { subcommand } => match subcommand {
            ExportCommand::Channel(opts) => {
                export::run_export(
                    &opts.channel,
                    opts.format.as_str(),
                    opts.output.as_deref(),
                    opts.workspace.as_deref(),
                )
                .await
            }
        },
        Command::Unreads { subcommand } => {
            let sub = subcommand.unwrap_or(UnreadsCommand::Show(UnreadsShowOptions {
                counts_only: false,
                max_messages: 10,
                max_body_chars: 4000,
                include_system: false,
                format: None,
                workspace: None,
            }));
            unreads::handle_unreads(sub).await
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
        Command::Slash { subcommand } => {
            let sub = subcommand.ok_or_else(|| {
                SlackersError::Other(
                    "Usage: slackers slash run --channel <CHANNEL> <COMMAND>...\n\
                     Provide --channel and the slash command to execute."
                        .to_string(),
                )
            })?;
            match sub {
                SlashCommand::Run(..) => {
                    check_read_only(read_only)?;
                    slash::handle_slash(sub).await
                }
            }
        }
        Command::Store { subcommand } => {
            store::handle_store(subcommand, read_only).await
        }
        Command::Watch(cmd) => {
            watch::handle_watch(cmd).await
        }
        Command::Sync { subcommand } => {
            sync_cmd::handle_sync(subcommand, read_only).await
        }
        Command::Query { subcommand } => {
            query::handle_query(subcommand).await
        }
        Command::Report { subcommand } => {
            report::handle_report(subcommand).await
        }
        Command::Config { subcommand } => {
            config::handle_config(subcommand).await
        }
        Command::Serve => {
            crate::mcp::run_server(read_only).await
        }
    }
}
