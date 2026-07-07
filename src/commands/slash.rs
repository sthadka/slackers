use crate::auth::resolve_auth;
use crate::cli::SlashCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::slash::{execute_slash_command, parse_slash_command};
use crate::slack::{resolve_channel_id, SlackClient};

pub async fn handle_slash(subcommand: SlashCommand) -> Result<()> {
    match subcommand {
        SlashCommand::Run(opts) => {
            handle_slash_run(&opts.channel, &opts.command, opts.workspace.as_deref()).await
        }
    }
}

async fn handle_slash_run(
    channel: &str,
    command_parts: &[String],
    workspace: Option<&str>,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let channel_id = resolve_channel_id(&client, channel).await?;

    let full_command = command_parts.join(" ");
    let (command, text) = parse_slash_command(&full_command)?;

    let result = execute_slash_command(&client, &channel_id, &command, &text).await?;

    if crate::output::is_quiet() {
        println!("{}", to_json_output(&serde_json::json!({ "ok": true })));
    } else {
        println!("{}", to_json_output(&result));
    }
    Ok(())
}
