use crate::tools::Tool;
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::process::Stdio;
use tokio::sync::Mutex;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use serde_json::Value;
use std::sync::OnceLock;
use std::sync::Mutex as StdMutex;

static MEMORY_MCP_CLIENT: OnceLock<StdMutex<Option<McpClient>>> = OnceLock::new();

pub fn set_memory_mcp_client(client: McpClient) {
    let cell = MEMORY_MCP_CLIENT.get_or_init(|| StdMutex::new(None));
    if let Ok(mut lock) = cell.lock() {
        *lock = Some(client);
    }
}

pub fn get_memory_mcp_client() -> Option<McpClient> {
    MEMORY_MCP_CLIENT.get().and_then(|cell| cell.lock().ok().and_then(|lock| lock.clone()))
}

pub mod mcp_grpc {
    tonic::include_proto!("mcp");
}

pub enum McpClientType {
    Stdio {
        #[allow(dead_code)]
        child: Child,
        stdout_reader: BufReader<tokio::process::ChildStdout>,
        stdin_writer: tokio::process::ChildStdin,
    },
    Grpc {
        #[allow(dead_code)]
        child: Option<Child>,
        client: mcp_grpc::mcp_service_client::McpServiceClient<tonic::transport::Channel>,
    }
}

pub struct McpClientInner {
    client_type: McpClientType,
    next_id: usize,
}

#[derive(Clone)]
pub struct McpClient(Arc<Mutex<Option<McpClientInner>>>);

impl McpClient {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self> {
        let mut grpc_port = None;
        if let Some(pos) = args.iter().position(|a| a == "--grpc") {
            if let Some(port_str) = args.get(pos + 1) {
                if let Ok(port) = port_str.parse::<u16>() {
                    grpc_port = Some(port);
                }
            }
        }

        if let Some(port) = grpc_port {
            let child = Command::new(command)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let channel = tonic::transport::Channel::from_shared(format!("http://127.0.0.1:{}", port))?
                .connect_timeout(std::time::Duration::from_secs(3))
                .connect()
                .await?;

            let mut grpc_client = mcp_grpc::mcp_service_client::McpServiceClient::new(channel);

            let init_params = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "openz",
                    "version": "0.1.0"
                }
            });

            let req = mcp_grpc::McpRequest {
                method: "initialize".to_string(),
                params_json: serde_json::to_string(&init_params)?,
                id: 1,
                has_id: true,
            };

            let _init_resp = grpc_client.call(req).await?;

            let notif_req = mcp_grpc::McpRequest {
                method: "notifications/initialized".to_string(),
                params_json: "{}".to_string(),
                id: 0,
                has_id: false,
            };

            grpc_client.call(notif_req).await?;

            let client = McpClientInner {
                client_type: McpClientType::Grpc {
                    child: Some(child),
                    client: grpc_client,
                },
                next_id: 2,
            };

            return Ok(McpClient(Arc::new(Mutex::new(Some(client)))));
        }

        // Automatic in-process gRPC bridge for all stdio servers to avoid stdio pollution
        let bridge_port = find_free_port();
        let cmd_string = command.to_string();
        let cmd_args = args.to_vec();

        tokio::spawn(async move {
            if let Err(e) = run_mcp_bridge(bridge_port, &cmd_string, &cmd_args).await {
                tracing::error!("In-process gRPC MCP bridge failed for {}: {:?}", cmd_string, e);
            }
        });

        // Robust retry connection loop to connect to the dynamic bridge port
        let mut client = None;
        for i in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            match tonic::transport::Channel::from_shared(format!("http://127.0.0.1:{}", bridge_port))?
                .connect_timeout(std::time::Duration::from_secs(1))
                .connect()
                .await
            {
                Ok(channel) => {
                    client = Some(mcp_grpc::mcp_service_client::McpServiceClient::new(channel));
                    break;
                }
                Err(e) => {
                    if i == 19 {
                        return Err(anyhow!("Failed to connect to local gRPC bridge for {} after 3s: {}", command, e));
                    }
                }
            }
        }
        let mut grpc_client = client.unwrap();

        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "openz",
                "version": "0.1.0"
            }
        });

        let req = mcp_grpc::McpRequest {
            method: "initialize".to_string(),
            params_json: serde_json::to_string(&init_params)?,
            id: 1,
            has_id: true,
        };

        let _init_resp = grpc_client.call(req).await?;

        let notif_req = mcp_grpc::McpRequest {
            method: "notifications/initialized".to_string(),
            params_json: "{}".to_string(),
            id: 0,
            has_id: false,
        };

        grpc_client.call(notif_req).await?;

        let client = McpClientInner {
            client_type: McpClientType::Grpc {
                child: None,
                client: grpc_client,
            },
            next_id: 2,
        };

        Ok(McpClient(Arc::new(Mutex::new(Some(client)))))
    }

    pub async fn list_tools(&self) -> Result<Vec<Value>> {
        let mut lock = self.0.lock().await;
        let client = lock.as_mut().ok_or_else(|| anyhow!("Client closed"))?;

        match &mut client.client_type {
            McpClientType::Grpc { client: grpc_client, .. } => {
                let req = mcp_grpc::McpRequest {
                    method: "tools/list".to_string(),
                    params_json: "{}".to_string(),
                    id: client.next_id as i64,
                    has_id: true,
                };
                client.next_id += 1;

                let resp = grpc_client.call(req).await?.into_inner();
                if !resp.error_json.is_empty() {
                    return Err(anyhow!("MCP error: {}", resp.error_json));
                }

                let result_val: Value = serde_json::from_str(&resp.result_json)?;
                let tools = result_val.get("tools")
                    .and_then(|t| t.as_array())
                    .ok_or_else(|| anyhow!("Invalid tools/list response"))?;

                Ok(tools.clone())
            }
            McpClientType::Stdio { stdin_writer, stdout_reader, .. } => {
                let req = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "tools/list",
                    "id": client.next_id
                });
                client.next_id += 1;

                let req_str = format!("{}\n", serde_json::to_string(&req)?);
                stdin_writer.write_all(req_str.as_bytes()).await?;
                stdin_writer.flush().await?;

                let mut line = String::new();
                stdout_reader.read_line(&mut line).await?;

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
        }
    }

    pub async fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        let mut lock = self.0.lock().await;
        let client = lock.as_mut().ok_or_else(|| anyhow!("Client closed"))?;

        match &mut client.client_type {
            McpClientType::Grpc { client: grpc_client, .. } => {
                let params = serde_json::json!({
                    "name": name,
                    "arguments": arguments
                });

                let req = mcp_grpc::McpRequest {
                    method: "tools/call".to_string(),
                    params_json: serde_json::to_string(&params)?,
                    id: client.next_id as i64,
                    has_id: true,
                };
                client.next_id += 1;

                let resp = grpc_client.call(req).await?.into_inner();
                if !resp.error_json.is_empty() {
                    return Err(anyhow!("MCP tool call error: {}", resp.error_json));
                }

                let result_val: Value = serde_json::from_str(&resp.result_json)?;
                Ok(result_val)
            }
            McpClientType::Stdio { stdin_writer, stdout_reader, .. } => {
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
                stdin_writer.write_all(req_str.as_bytes()).await?;
                stdin_writer.flush().await?;

                let mut line = String::new();
                stdout_reader.read_line(&mut line).await?;

                let resp: Value = serde_json::from_str(&line)?;
                if let Some(error) = resp.get("error") {
                    return Err(anyhow!("MCP tool call error: {}", error));
                }

                let result = resp.get("result")
                    .ok_or_else(|| anyhow!("Invalid tools/call response"))?;

                Ok(result.clone())
            }
        }
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

// -------------------- gRPC MCP BRIDGE IMPLEMENTATION --------------------

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|addr| addr.port())
        .unwrap_or(50060)
}

pub struct McpBridgeService {
    writer: Arc<Mutex<tokio::process::ChildStdin>>,
    reader: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
}

#[tonic::async_trait]
impl mcp_grpc::mcp_service_server::McpService for McpBridgeService {
    async fn call(&self, request: tonic::Request<mcp_grpc::McpRequest>) -> std::result::Result<tonic::Response<mcp_grpc::McpResponse>, tonic::Status> {
        let req = request.into_inner();
        
        let rpc_req = if !req.has_id {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": req.method,
                "params": serde_json::from_str::<serde_json::Value>(&req.params_json).unwrap_or(serde_json::Value::Null)
            })
        } else {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "method": req.method,
                "params": serde_json::from_str::<serde_json::Value>(&req.params_json).unwrap_or(serde_json::Value::Null)
            })
        };
        
        let req_str = format!("{}\n", serde_json::to_string(&rpc_req).map_err(|e| tonic::Status::invalid_argument(e.to_string()))?);
        
        let mut writer_lock = self.writer.lock().await;
        let mut reader_lock = self.reader.lock().await;
        
        writer_lock.write_all(req_str.as_bytes()).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        writer_lock.flush().await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        
        if !req.has_id {
            return Ok(tonic::Response::new(mcp_grpc::McpResponse {
                result_json: String::new(),
                error_json: String::new(),
                id: 0,
            }));
        }
        
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = reader_lock.read_line(&mut line).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
            if bytes_read == 0 {
                return Err(tonic::Status::aborted("Child process exited / closed stdout pipe"));
            }
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(resp_val) = serde_json::from_str::<serde_json::Value>(&line) {
                if resp_val.get("jsonrpc").is_some() && (resp_val.get("id").is_some() || resp_val.get("result").is_some() || resp_val.get("error").is_some()) {
                    let id = resp_val.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                    let result_json = resp_val.get("result").map(|v| v.to_string()).unwrap_or_default();
                    let error_json = resp_val.get("error").map(|v| v.to_string()).unwrap_or_default();
                    
                    return Ok(tonic::Response::new(mcp_grpc::McpResponse {
                        result_json,
                        error_json,
                        id,
                    }));
                }
            }
            // Filter out non-JSON-RPC stdout messages
            tracing::warn!("Filtered non-JSON-RPC stdio output from bridge: {}", line.trim());
        }
    }
}

pub async fn run_mcp_bridge(port: u16, command: &str, args: &[String]) -> Result<()> {
    tracing::info!("Launching target MCP server: {} {:?}", command, args);
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to open child stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("Failed to open child stdout"))?;

    let service = McpBridgeService {
        writer: Arc::new(Mutex::new(stdin)),
        reader: Arc::new(Mutex::new(BufReader::new(stdout))),
    };

    let addr = format!("127.0.0.1:{}", port).parse()?;
    tracing::info!("gRPC MCP Bridge listening on {}", addr);

    tonic::transport::Server::builder()
        .add_service(mcp_grpc::mcp_service_server::McpServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
