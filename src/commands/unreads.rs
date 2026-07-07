use crate::auth::resolve_auth;
use crate::cli::UnreadsCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::render::format::OutputFormat;
use crate::slack::unreads::{fetch_unreads, FetchUnreadsOptions};
use crate::slack::SlackClient;

pub async fn handle_unreads(subcommand: UnreadsCommand) -> Result<()> {
    match subcommand {
        UnreadsCommand::Show(opts) => handle_unreads_show(opts).await,
    }
}

async fn handle_unreads_show(opts: crate::cli::UnreadsShowOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let fetch_opts = FetchUnreadsOptions {
        include_messages: !opts.counts_only,
        max_messages_per_channel: opts.max_messages,
        max_body_chars: opts.max_body_chars,
        skip_system_messages: !opts.include_system,
    };

    let result = fetch_unreads(&client, &fetch_opts).await?;

    let format = opts
        .format
        .as_deref()
        .map(|f| OutputFormat::from_str(f))
        .flatten()
        .unwrap_or_default();

    match format {
        OutputFormat::Json => {
            println!("{}", to_json_output(&result));
        }
        _ => {
            let headers = &[
                "Channel",
                "Type",
                "Unreads",
                "Mentions",
            ];
            let rows: Vec<Vec<String>> = result
                .channels
                .iter()
                .map(|ch| {
                    vec![
                        ch.channel_name
                            .clone()
                            .unwrap_or_else(|| ch.channel_id.clone()),
                        ch.channel_type.clone(),
                        ch.unread_count.to_string(),
                        ch.mention_count.to_string(),
                    ]
                })
                .collect();
            println!("{}", format.render_rows(headers, &rows));
        }
    }

    Ok(())
}
