use crate::error::{Result, SlackersError};
use crate::slack::SlackClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelWorkflow {
    pub title: String,
    pub trigger_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelWorkflows {
    pub channel_id: String,
    pub workflows: Vec<ChannelWorkflow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPreview {
    pub trigger_id: String,
    #[serde(rename = "type")]
    pub trigger_type: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut_url: Option<String>,
    pub workflow: WorkflowInfo,
    pub collaborators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInfo {
    pub id: String,
    pub title: String,
    pub description: String,
    pub app_id: String,
    pub app_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunResult {
    pub function_execution_id: String,
    pub trigger_execution_id: String,
    pub is_slow_workflow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub name: String,
    pub title: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSchema {
    pub workflow_id: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_title: Option<String>,
    pub fields: Vec<FormField>,
    pub steps: Vec<String>,
}

pub async fn list_channel_workflows(
    client: &SlackClient,
    channel_id: &str,
) -> Result<ChannelWorkflows> {
    let (bookmarked, featured) = tokio::join!(
        list_bookmarked_workflows(client, channel_id),
        list_featured_workflows(client, channel_id)
    );

    let bookmarked = bookmarked?;
    let featured = featured.unwrap_or_default();

    let featured_ids: std::collections::HashSet<String> =
        featured.iter().map(|f| f.trigger_id.clone()).collect();

    let mut seen = std::collections::HashSet::new();
    let mut workflows = Vec::new();

    for bk in &bookmarked {
        if let Some(ref tid) = bk.trigger_id {
            seen.insert(tid.clone());
        }
        workflows.push(ChannelWorkflow {
            title: bk.title.clone(),
            trigger_id: bk.trigger_id.clone().unwrap_or_default(),
            link: bk.link.clone(),
            app_id: bk.app_id.clone(),
            featured: bk
                .trigger_id
                .as_ref()
                .map(|tid| featured_ids.contains(tid))
                .unwrap_or(false),
        });
    }

    for ft in &featured {
        if !seen.contains(&ft.trigger_id) {
            workflows.push(ChannelWorkflow {
                title: ft.title.clone(),
                trigger_id: ft.trigger_id.clone(),
                link: None,
                app_id: None,
                featured: true,
            });
        }
    }

    Ok(ChannelWorkflows {
        channel_id: channel_id.to_string(),
        workflows,
    })
}

struct BookmarkedWorkflow {
    title: String,
    trigger_id: Option<String>,
    link: Option<String>,
    app_id: Option<String>,
}

async fn list_bookmarked_workflows(
    client: &SlackClient,
    channel_id: &str,
) -> Result<Vec<BookmarkedWorkflow>> {
    let params = vec![("channel_id".to_string(), channel_id.to_string())];
    let resp = client.api_call("bookmarks.list", params).await?;

    let bookmarks = resp
        .get("bookmarks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let results: Vec<BookmarkedWorkflow> = bookmarks
        .iter()
        .filter(|b| {
            let link = b.get("link").and_then(|v| v.as_str()).unwrap_or("");
            let shortcut_id = b.get("shortcut_id").and_then(|v| v.as_str());
            shortcut_id.is_some() || link.contains("slack.com/shortcuts/")
        })
        .map(|b| {
            let link = b.get("link").and_then(|v| v.as_str()).map(|s| s.to_string());
            let shortcut_id = b
                .get("shortcut_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let trigger_id = shortcut_id.or_else(|| extract_trigger_id(link.as_deref()));

            BookmarkedWorkflow {
                title: b
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                trigger_id,
                link,
                app_id: b.get("app_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }
        })
        .collect();

    Ok(results)
}

struct FeaturedWorkflow {
    trigger_id: String,
    title: String,
}

async fn list_featured_workflows(
    client: &SlackClient,
    channel_id: &str,
) -> Result<Vec<FeaturedWorkflow>> {
    let channel_ids_json = serde_json::to_string(&vec![channel_id])
        .unwrap_or_else(|_| format!("[\"{}\"]", channel_id));

    let params = vec![("channel_ids".to_string(), channel_ids_json)];
    let resp = match client.api_call("workflows.featured.list", params).await {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()),
    };

    let entries = resp
        .get("featured_workflows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let entry = entries.iter().find(|e| {
        e.get("channel_id")
            .and_then(|v| v.as_str())
            .map(|id| id == channel_id)
            .unwrap_or(false)
    });

    let entry = match entry {
        Some(e) => e,
        None => return Ok(Vec::new()),
    };

    let triggers = entry
        .get("triggers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let results: Vec<FeaturedWorkflow> = triggers
        .iter()
        .filter_map(|t| {
            let trigger_id = t.get("id").and_then(|v| v.as_str())?.to_string();
            if trigger_id.is_empty() {
                return None;
            }
            Some(FeaturedWorkflow {
                trigger_id,
                title: t
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect();

    Ok(results)
}

pub async fn preview_workflow(
    client: &SlackClient,
    trigger_id: &str,
) -> Result<WorkflowPreview> {
    let params = vec![("trigger_ids".to_string(), trigger_id.to_string())];
    let resp = client
        .api_call("workflows.triggers.preview", params)
        .await?;

    let triggers = resp
        .get("triggers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if triggers.is_empty() {
        let rejected = resp
            .get("rejected_triggers")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if rejected {
            return Err(SlackersError::Other(format!(
                "Trigger {} was rejected -- you may not have access",
                trigger_id
            )));
        }
        return Err(SlackersError::Other(format!(
            "No preview data returned for trigger {}",
            trigger_id
        )));
    }

    let t = &triggers[0];
    let wf = t.get("workflow").unwrap_or(&Value::Null);
    let wf_app = wf.get("app").unwrap_or(&Value::Null);
    let details = t.get("workflow_details").unwrap_or(&Value::Null);

    let collaborators: Vec<String> = details
        .get("collaborators")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    Ok(WorkflowPreview {
        trigger_id: get_str(t, "id").unwrap_or_else(|| trigger_id.to_string()),
        trigger_type: get_str(t, "type").unwrap_or_default(),
        name: get_str(t, "name").unwrap_or_default(),
        description: get_str(t, "description").unwrap_or_default(),
        shortcut_url: get_str(t, "shortcut_url"),
        workflow: WorkflowInfo {
            id: get_str(wf, "workflow_id").unwrap_or_default(),
            title: get_str(wf, "title").unwrap_or_default(),
            description: get_str(wf, "description").unwrap_or_default(),
            app_id: get_str(wf, "app_id")
                .or_else(|| get_str(wf_app, "id"))
                .unwrap_or_default(),
            app_name: get_str(wf_app, "name").unwrap_or_default(),
        },
        collaborators,
    })
}

pub async fn get_workflow_schema(
    client: &SlackClient,
    workflow_id: &str,
) -> Result<WorkflowSchema> {
    let params = vec![("workflow_id".to_string(), workflow_id.to_string())];
    let resp = client.api_call("workflows.get", params).await?;

    let wf = resp
        .get("workflow")
        .ok_or_else(|| SlackersError::Other(format!("No workflow found for ID {}", workflow_id)))?;

    let steps = wf
        .get("steps")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut step_summaries = Vec::new();
    let mut fields = Vec::new();
    let mut form_title: Option<String> = None;

    for step in &steps {
        let func = step.get("function").unwrap_or(&Value::Null);
        let callback_id = get_str(func, "callback_id").unwrap_or_default();
        let title = get_str(func, "title").unwrap_or_else(|| callback_id.clone());
        step_summaries.push(title);

        if callback_id == "open_form" {
            let inputs = step.get("inputs").unwrap_or(&Value::Null);
            let title_input = inputs.get("title").unwrap_or(&Value::Null);
            form_title = get_str(title_input, "value");

            let fields_input = inputs.get("fields").unwrap_or(&Value::Null);
            let fields_value = fields_input.get("value").unwrap_or(&Value::Null);

            let elements = fields_value
                .get("elements")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let required_set: std::collections::HashSet<String> = fields_value
                .get("required")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|r| r.as_str().map(|s| s.to_string()))
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();

            fields = elements
                .iter()
                .map(|el| {
                    let name = get_str(el, "name").unwrap_or_default();
                    let is_required = required_set.contains(&name);
                    let is_long = el
                        .get("long")
                        .and_then(|v| v.as_bool())
                        .filter(|&b| b);

                    FormField {
                        name,
                        title: get_str(el, "title").unwrap_or_default(),
                        field_type: get_str(el, "type").unwrap_or_else(|| "string".to_string()),
                        description: get_str(el, "description").unwrap_or_default(),
                        required: is_required,
                        long: is_long,
                    }
                })
                .collect();
        }
    }

    Ok(WorkflowSchema {
        workflow_id: get_str(wf, "id").unwrap_or_else(|| workflow_id.to_string()),
        title: get_str(wf, "title").unwrap_or_default(),
        description: get_str(wf, "description").unwrap_or_default(),
        form_title,
        fields,
        steps: step_summaries,
    })
}

pub async fn resolve_shortcut_url(
    client: &SlackClient,
    channel_id: &str,
    trigger_id: &str,
) -> Result<String> {
    let params = vec![("channel_id".to_string(), channel_id.to_string())];
    let resp = client.api_call("bookmarks.list", params).await?;

    let bookmarks = resp
        .get("bookmarks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for b in &bookmarks {
        let shortcut_id = b.get("shortcut_id").and_then(|v| v.as_str());
        if shortcut_id == Some(trigger_id) {
            if let Some(link) = b.get("link").and_then(|v| v.as_str()) {
                return Ok(link.to_string());
            }
        }
    }

    Err(SlackersError::Other(format!(
        "Could not find shortcut URL for trigger {} in channel bookmarks",
        trigger_id
    )))
}

pub async fn run_workflow(
    client: &SlackClient,
    shortcut_url: &str,
    channel_id: &str,
    trigger_id: &str,
) -> Result<WorkflowRunResult> {
    let client_token = format!(
        "cli-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let context = serde_json::json!({
        "location": "bookmark",
        "channel_id": channel_id,
        "trigger_id": trigger_id,
    });

    let params = vec![
        ("url".to_string(), shortcut_url.to_string()),
        ("client_token".to_string(), client_token),
        ("context".to_string(), context.to_string()),
        ("run_precheck".to_string(), "true".to_string()),
    ];

    let resp = client
        .api_call("workflows.triggers.trip", params)
        .await?;

    Ok(WorkflowRunResult {
        function_execution_id: get_str(&resp, "function_execution_id").unwrap_or_default(),
        trigger_execution_id: get_str(&resp, "trigger_execution_id").unwrap_or_default(),
        is_slow_workflow: resp
            .get("is_slow_workflow")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    })
}

fn get_str(val: &Value, key: &str) -> Option<String> {
    val.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_trigger_id(link: Option<&str>) -> Option<String> {
    let link = link?;
    let re = regex::Regex::new(r"slack\.com/shortcuts/(Ft[A-Za-z0-9]+)").ok()?;
    re.captures(link)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_trigger_id_valid() {
        let link = "https://myteam.slack.com/shortcuts/Ft123ABC456/abc123";
        assert_eq!(
            extract_trigger_id(Some(link)),
            Some("Ft123ABC456".to_string())
        );
    }

    #[test]
    fn test_extract_trigger_id_no_match() {
        let link = "https://slack.com/other/path";
        assert_eq!(extract_trigger_id(Some(link)), None);
    }

    #[test]
    fn test_extract_trigger_id_none() {
        assert_eq!(extract_trigger_id(None), None);
    }

    #[test]
    fn test_channel_workflow_serialization() {
        let wf = ChannelWorkflow {
            title: "My Workflow".to_string(),
            trigger_id: "Ft123".to_string(),
            link: Some("https://slack.com/shortcuts/Ft123/abc".to_string()),
            app_id: Some("A123".to_string()),
            featured: true,
        };

        let json = serde_json::to_value(&wf).unwrap();
        assert_eq!(json["title"], "My Workflow");
        assert_eq!(json["trigger_id"], "Ft123");
        assert_eq!(json["featured"], true);
    }

    #[test]
    fn test_channel_workflow_skip_none_fields() {
        let wf = ChannelWorkflow {
            title: "Test".to_string(),
            trigger_id: "Ft456".to_string(),
            link: None,
            app_id: None,
            featured: false,
        };

        let json = serde_json::to_value(&wf).unwrap();
        assert!(json.get("link").is_none());
        assert!(json.get("app_id").is_none());
    }

    #[test]
    fn test_workflow_preview_serialization() {
        let preview = WorkflowPreview {
            trigger_id: "Ft123".to_string(),
            trigger_type: "shortcut".to_string(),
            name: "Test Flow".to_string(),
            description: "A test".to_string(),
            shortcut_url: Some("https://slack.com/shortcuts/Ft123".to_string()),
            workflow: WorkflowInfo {
                id: "Wf123".to_string(),
                title: "Test Workflow".to_string(),
                description: "Test".to_string(),
                app_id: "A123".to_string(),
                app_name: "TestApp".to_string(),
            },
            collaborators: vec!["U123".to_string()],
        };

        let json = serde_json::to_value(&preview).unwrap();
        assert_eq!(json["trigger_id"], "Ft123");
        assert_eq!(json["type"], "shortcut");
        assert_eq!(json["workflow"]["id"], "Wf123");
        assert_eq!(json["collaborators"][0], "U123");
    }

    #[test]
    fn test_workflow_schema_serialization() {
        let schema = WorkflowSchema {
            workflow_id: "Wf123".to_string(),
            title: "My Workflow".to_string(),
            description: "A test workflow".to_string(),
            form_title: Some("Submit Form".to_string()),
            fields: vec![FormField {
                name: "reason".to_string(),
                title: "Reason".to_string(),
                field_type: "string".to_string(),
                description: "Why?".to_string(),
                required: true,
                long: Some(true),
            }],
            steps: vec!["open_form".to_string(), "send_message".to_string()],
        };

        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["workflow_id"], "Wf123");
        assert_eq!(json["fields"][0]["name"], "reason");
        assert_eq!(json["fields"][0]["type"], "string");
        assert_eq!(json["fields"][0]["required"], true);
        assert_eq!(json["steps"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_workflow_run_result_serialization() {
        let result = WorkflowRunResult {
            function_execution_id: "Fn123".to_string(),
            trigger_execution_id: "Tx456".to_string(),
            is_slow_workflow: true,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["function_execution_id"], "Fn123");
        assert_eq!(json["is_slow_workflow"], true);
    }

    #[test]
    fn test_get_str_helper() {
        let val = json!({"name": "test", "count": 42});
        assert_eq!(get_str(&val, "name"), Some("test".to_string()));
        assert_eq!(get_str(&val, "count"), None);
        assert_eq!(get_str(&val, "missing"), None);
    }
}
