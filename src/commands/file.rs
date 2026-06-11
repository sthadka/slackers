use crate::auth::resolve_auth;
use crate::cli::{FileCommand, FileDeleteOptions, FileListOptions, FileUploadOptions};
use crate::error::{Result, SlackersError};
use crate::output::to_json_output;
use crate::slack::SlackClient;
use serde_json::json;
use std::path::Path;

pub async fn handle_file(subcommand: FileCommand) -> Result<()> {
    match subcommand {
        FileCommand::Upload(opts) => handle_file_upload(opts).await,
        FileCommand::Delete(opts) => handle_file_delete(opts).await,
        FileCommand::List(opts) => handle_file_list(opts).await,
    }
}

async fn handle_file_upload(opts: FileUploadOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let file_path = Path::new(&opts.file);

    let resp = client
        .upload_file(
            file_path,
            opts.channels.unwrap_or_default(),
            opts.comment,
            opts.title,
            opts.filename,
        )
        .await?;

    let output = json!({
        "ok": true,
        "file_id": resp.id,
        "permalink": resp.permalink,
        "url_private": resp.url_private,
    });

    println!("{}", to_json_output(&output));
    Ok(())
}

async fn handle_file_delete(opts: FileDeleteOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    client.delete_file(&opts.file_id).await?;

    let output = json!({ "ok": true });
    println!("{}", to_json_output(&output));
    Ok(())
}

async fn handle_file_list(opts: FileListOptions) -> Result<()> {
    let auth_result = resolve_auth(opts.workspace.as_deref())?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let files = list_files(&client, opts.channel.as_deref(), opts.limit).await?;

    println!("{}", to_json_output(&files));
    Ok(())
}

/// Call files.list and return a compact array of file objects.
async fn list_files(
    client: &SlackClient,
    channel: Option<&str>,
    limit: u32,
) -> Result<Vec<serde_json::Value>> {
    let mut params = vec![
        ("count".to_string(), limit.to_string()),
    ];

    if let Some(ch) = channel {
        params.push(("channel".to_string(), ch.to_string()));
    }

    let response = client.api_call("files.list", params).await?;

    let files_array = response
        .get("files")
        .and_then(|v| v.as_array())
        .ok_or_else(|| SlackersError::Other("No 'files' field in files.list response".to_string()))?;

    let compact: Vec<serde_json::Value> = files_array
        .iter()
        .map(|f| {
            json!({
                "id": f.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "name": f.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "permalink": f.get("permalink").and_then(|v| v.as_str()),
                "size": f.get("size").and_then(|v| v.as_u64()),
                "filetype": f.get("filetype").and_then(|v| v.as_str()),
            })
        })
        .collect();

    Ok(compact)
}
