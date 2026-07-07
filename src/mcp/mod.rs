mod protocol;
mod tools;

use protocol::{JsonRpcRequest, JsonRpcResponse, INVALID_PARAMS, METHOD_NOT_FOUND};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use tools::{all_tools, tool_to_cli_args};

pub async fn run_server(read_only: bool) -> crate::error::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    None,
                    -32700,
                    format!("Parse error: {}", e),
                );
                write_response(&mut stdout, &resp)?;
                continue;
            }
        };

        if req.method == "notifications/initialized" || req.method.starts_with("notifications/") {
            continue;
        }

        let resp = handle_request(&req, read_only).await;
        write_response(&mut stdout, &resp)?;
    }

    Ok(())
}

fn write_response(stdout: &mut io::Stdout, resp: &JsonRpcResponse) -> io::Result<()> {
    let json = serde_json::to_string(resp)?;
    writeln!(stdout, "{}", json)?;
    stdout.flush()?;
    Ok(())
}

async fn handle_request(req: &JsonRpcRequest, read_only: bool) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => {
            JsonRpcResponse::success(
                req.id.clone(),
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "slackers",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            )
        }
        "tools/list" => {
            let tools_defs = all_tools();
            let tools_json: Vec<Value> = tools_defs
                .iter()
                .filter(|t| !read_only || !t.is_write)
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })
                })
                .collect();

            JsonRpcResponse::success(req.id.clone(), json!({ "tools": tools_json }))
        }
        "tools/call" => {
            handle_tool_call(req, read_only).await
        }
        _ => {
            JsonRpcResponse::error(
                req.id.clone(),
                METHOD_NOT_FOUND,
                format!("Method not found: {}", req.method),
            )
        }
    }
}

async fn handle_tool_call(req: &JsonRpcRequest, read_only: bool) -> JsonRpcResponse {
    let params = match &req.params {
        Some(p) => p,
        None => {
            return JsonRpcResponse::error(
                req.id.clone(),
                INVALID_PARAMS,
                "Missing params".to_string(),
            );
        }
    };

    let tool_name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            return JsonRpcResponse::error(
                req.id.clone(),
                INVALID_PARAMS,
                "Missing tool name".to_string(),
            );
        }
    };

    let tools_defs = all_tools();
    let tool_def = tools_defs.iter().find(|t| t.name == tool_name);

    if let Some(def) = tool_def {
        if read_only && def.is_write {
            return JsonRpcResponse::success(
                req.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": "Error: Operation blocked — --read-only mode is enabled"
                    }],
                    "isError": true
                }),
            );
        }
    } else {
        return JsonRpcResponse::success(
            req.id.clone(),
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("Error: Unknown tool '{}'", tool_name)
                }],
                "isError": true
            }),
        );
    }

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(json!({}));

    let cli_args = match tool_to_cli_args(tool_name, &arguments) {
        Some(args) => args,
        None => {
            return JsonRpcResponse::success(
                req.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: Could not map tool '{}' to CLI args", tool_name)
                    }],
                    "isError": true
                }),
            );
        }
    };

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::success(
                req.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: Could not find executable path: {}", e)
                    }],
                    "isError": true
                }),
            );
        }
    };

    match tokio::process::Command::new(&exe)
        .args(&cli_args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
    {
        Ok(output) => {
            let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
            let is_error = !output.status.success();

            let text = if is_error && !stderr_str.is_empty() {
                if stdout_str.is_empty() {
                    stderr_str
                } else {
                    format!("{}\n{}", stdout_str, stderr_str)
                }
            } else {
                stdout_str
            };

            JsonRpcResponse::success(
                req.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": text.trim()
                    }],
                    "isError": is_error
                }),
            )
        }
        Err(e) => {
            JsonRpcResponse::success(
                req.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: Failed to execute command: {}", e)
                    }],
                    "isError": true
                }),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::JsonRpcRequest;

    #[tokio::test]
    async fn test_initialize() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "1.0"}
            })),
        };
        let resp = handle_request(&req, false).await;
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "slackers");
    }

    #[tokio::test]
    async fn test_tools_list() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = handle_request(&req, false).await;
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.len() > 40);
    }

    #[tokio::test]
    async fn test_tools_list_read_only_filters_write_tools() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp_full = handle_request(&req, false).await;
        let resp_ro = handle_request(&req, true).await;

        let full_tools = resp_full.result.unwrap()["tools"].as_array().unwrap().len();
        let ro_tools = resp_ro.result.unwrap()["tools"].as_array().unwrap().len();
        assert!(ro_tools < full_tools);
    }

    #[tokio::test]
    async fn test_tool_call_read_only_blocks_write() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "message_send",
                "arguments": {"target": "#general", "text": "hi"}
            })),
        };
        let resp = handle_request(&req, true).await;
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
        assert!(result["content"][0]["text"].as_str().unwrap().contains("read-only"));
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(4)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "nonexistent",
                "arguments": {}
            })),
        };
        let resp = handle_request(&req, false).await;
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
        assert!(result["content"][0]["text"].as_str().unwrap().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(5)),
            method: "foo/bar".to_string(),
            params: None,
        };
        let resp = handle_request(&req, false).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }
}
