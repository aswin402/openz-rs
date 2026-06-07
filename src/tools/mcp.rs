use crate::tools::Tool;
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::process::Stdio;
use tokio::sync::Mutex;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use serde_json::Value;

pub struct McpClientInner {
    #[allow(dead_code)]
    child: Child,
    stdout_reader: BufReader<tokio::process::ChildStdout>,
    stdin_writer: tokio::process::ChildStdin,
    next_id: usize,
}

#[derive(Clone)]
pub struct McpClient(Arc<Mutex<Option<McpClientInner>>>);

impl McpClient {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin_writer = child.stdin.take().ok_or_else(|| anyhow!("Failed to open stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("Failed to open stdout"))?;
        let stdout_reader = BufReader::new(stdout);

        let mut client = McpClientInner {
            child,
            stdout_reader,
            stdin_writer,
            next_id: 1,
        };

        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "id": client.next_id,
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "openz",
                    "version": "0.1.0"
                }
            }
        });
        client.next_id += 1;

        let req_str = format!("{}\n", serde_json::to_string(&init_req)?);
        client.stdin_writer.write_all(req_str.as_bytes()).await?;
        client.stdin_writer.flush().await?;

        let mut line = String::new();
        client.stdout_reader.read_line(&mut line).await?;
        
        let init_notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let notif_str = format!("{}\n", serde_json::to_string(&init_notif)?);
        client.stdin_writer.write_all(notif_str.as_bytes()).await?;
        client.stdin_writer.flush().await?;

        Ok(McpClient(Arc::new(Mutex::new(Some(client)))))
    }

    pub async fn list_tools(&self) -> Result<Vec<Value>> {
        let mut lock = self.0.lock().await;
        let client = lock.as_mut().ok_or_else(|| anyhow!("Client closed"))?;

        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": client.next_id
        });
        client.next_id += 1;

        let req_str = format!("{}\n", serde_json::to_string(&req)?);
        client.stdin_writer.write_all(req_str.as_bytes()).await?;
        client.stdin_writer.flush().await?;

        let mut line = String::new();
        client.stdout_reader.read_line(&mut line).await?;

        let resp: Value = serde_json::from_str(&line)?;
        if let Some(error) = resp.get("error") {
            return Err(anyhow!("MCP error: {}", error));
        }

        let tools = resp.get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .ok_or_else(|| anyhow!("Invalid tools/list response"))?;

        Ok(tools.clone())
    }

    pub async fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        let mut lock = self.0.lock().await;
        let client = lock.as_mut().ok_or_else(|| anyhow!("Client closed"))?;

        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "id": client.next_id,
            "params": {
                "name": name,
                "arguments": arguments
            }
        });
        client.next_id += 1;

        let req_str = format!("{}\n", serde_json::to_string(&req)?);
        client.stdin_writer.write_all(req_str.as_bytes()).await?;
        client.stdin_writer.flush().await?;

        let mut line = String::new();
        client.stdout_reader.read_line(&mut line).await?;

        let resp: Value = serde_json::from_str(&line)?;
        if let Some(error) = resp.get("error") {
            return Err(anyhow!("MCP tool call error: {}", error));
        }

        let result = resp.get("result")
            .ok_or_else(|| anyhow!("Invalid tools/call response"))?;

        Ok(result.clone())
    }
}

pub struct McpToolWrapper {
    pub client: McpClient,
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[async_trait::async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> Value {
        self.parameters.clone()
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        self.client.call_tool(&self.name, arguments).await
    }
}
