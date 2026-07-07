use crate::auth::resolve_auth;
use crate::cli::CanvasCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::{fetch_canvas, parse_canvas_identifier, SlackClient};
use serde_json::json;

pub async fn handle_canvas(subcommand: CanvasCommand) -> Result<()> {
    match subcommand {
        CanvasCommand::Get {
            canvas,
            workspace,
            max_body_chars,
        } => handle_canvas_get(&canvas, workspace.as_deref(), max_body_chars).await,
    }
}

async fn handle_canvas_get(
    target: &str,
    workspace: Option<&str>,
    max_body_chars: i32,
) -> Result<()> {
    let file_id = parse_canvas_identifier(target)?;

    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth.clone(), auth_result.workspace_url);

    let max = if max_body_chars < 0 {
        None
    } else {
        Some(max_body_chars as usize)
    };

    // Fetch and convert canvas
    let markdown = fetch_canvas(&client, &auth_result.auth, &file_id, max).await?;

    let char_count = markdown.len();
    let truncated = max.map(|m| char_count >= m).unwrap_or(false);

    // Build output
    let output = json!({
        "file_id": file_id,
        "markdown_content": markdown,
        "char_count": char_count,
        "truncated": truncated
    });

    println!("{}", to_json_output(&output));
    Ok(())
}
