use crate::auth::resolve_auth;
use crate::cli::WorkflowCommand;
use crate::error::Result;
use crate::output::to_json_output;
use crate::slack::workflows::{
    get_workflow_schema, list_channel_workflows, preview_workflow, resolve_shortcut_url,
    run_workflow,
};
use crate::slack::{resolve_channel_id, SlackClient};

pub async fn handle_workflow(subcommand: WorkflowCommand) -> Result<()> {
    match subcommand {
        WorkflowCommand::List {
            channel,
            workspace,
        } => handle_workflow_list(&channel, workspace.as_deref()).await,
        WorkflowCommand::Preview {
            trigger_id,
            workspace,
        } => handle_workflow_preview(&trigger_id, workspace.as_deref()).await,
        WorkflowCommand::Get { id, workspace } => {
            handle_workflow_get(&id, workspace.as_deref()).await
        }
        WorkflowCommand::Run {
            trigger_id,
            channel,
            workspace,
        } => handle_workflow_run(&trigger_id, &channel, workspace.as_deref()).await,
    }
}

async fn handle_workflow_list(channel: &str, workspace: Option<&str>) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let channel_id = resolve_channel_id(&client, channel).await?;
    let result = list_channel_workflows(&client, &channel_id).await?;

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_workflow_preview(trigger_id: &str, workspace: Option<&str>) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let result = preview_workflow(&client, trigger_id).await?;

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_workflow_get(id: &str, workspace: Option<&str>) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let workflow_id = if id.starts_with("Ft") {
        let preview = preview_workflow(&client, id).await?;
        preview.workflow.id
    } else {
        id.to_string()
    };

    let result = get_workflow_schema(&client, &workflow_id).await?;

    println!("{}", to_json_output(&result));
    Ok(())
}

async fn handle_workflow_run(
    trigger_id: &str,
    channel: &str,
    workspace: Option<&str>,
) -> Result<()> {
    let auth_result = resolve_auth(workspace)?;
    let client = SlackClient::new(auth_result.auth, auth_result.workspace_url);

    let channel_id = resolve_channel_id(&client, channel).await?;
    let shortcut_url = resolve_shortcut_url(&client, &channel_id, trigger_id).await?;
    let result = run_workflow(&client, &shortcut_url, &channel_id, trigger_id).await?;

    println!("{}", to_json_output(&result));
    Ok(())
}
