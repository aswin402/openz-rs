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

pub fn clear_memory_mcp_client() {
    if let Some(cell) = MEMORY_MCP_CLIENT.get() {
        if let Ok(mut lock) = cell.lock() {
            *lock = None;
        }
    }
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
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for McpClientInner {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        match &mut self.client_type {
            McpClientType::Grpc { ref mut child, .. } => {
                if let Some(ref mut c) = child {
                    let _ = c.start_kill();
                }
            }
            McpClientType::Stdio { ref mut child, .. } => {
                let _ = child.start_kill();
            }
        }
    }
}

#[derive(Clone)]
pub struct McpClient(Arc<Mutex<Option<McpClientInner>>>);

impl McpClient {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self> {
        let cache_key = format!("{}:{}", command, args.join(" "));
        static SPAWNED_MCP_CLIENTS: OnceLock<Mutex<std::collections::HashMap<String, McpClient>>> = OnceLock::new();
        let cell = SPAWNED_MCP_CLIENTS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
        let mut lock = cell.lock().await;
        if let Some(client) = lock.get(&cache_key) {
            return Ok(client.clone());
        }

        let cmd = command.to_string();
        let args_vec = args.to_vec();

        let fut = async move {
            let mut grpc_port = None;
            if let Some(pos) = args_vec.iter().position(|a| a == "--grpc") {
                if let Some(port_str) = args_vec.get(pos + 1) {
                    if let Ok(port) = port_str.parse::<u16>() {
                        grpc_port = Some(port);
                    }
                }
            }

            if let Some(port) = grpc_port {
                let child = Command::new(&cmd)
                    .args(&args_vec)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
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
                    shutdown_tx: None,
                };

                return Ok(McpClient(Arc::new(Mutex::new(Some(client)))));
            }

            // Automatic in-process gRPC bridge for all stdio servers to avoid stdio pollution
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            let bridge_port = find_free_port();
            let cmd_string = cmd.clone();
            let cmd_args = args_vec.clone();

            tokio::spawn(async move {
                if let Err(e) = run_mcp_bridge(bridge_port, &cmd_string, &cmd_args, shutdown_rx).await {
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
                            return Err(anyhow!("Failed to connect to local gRPC bridge for {} after 3s: {}", cmd, e));
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
                shutdown_tx: Some(shutdown_tx),
            };

            Ok(McpClient(Arc::new(Mutex::new(Some(client)))))
        };

        let result = match tokio::time::timeout(std::time::Duration::from_secs(15), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP spawn for {} timed out after 15s", command)),
        };

        match result {
            Ok(client) => {
                lock.insert(cache_key, client.clone());
                Ok(client)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<Value>> {
        let self_clone = self.clone();
        let fut = async move {
            let mut lock = self_clone.0.lock().await;
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
        };

        match tokio::time::timeout(std::time::Duration::from_secs(15), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP list_tools timed out after 15s")),
        }
    }

    pub async fn call_tool(&self, name: &str, arguments: &Value) -> Result<Value> {
        let self_clone = self.clone();
        let name_str = name.to_string();
        let args_val = arguments.clone();

        let fut = async move {
            let mut lock = self_clone.0.lock().await;
            let client = lock.as_mut().ok_or_else(|| anyhow!("Client closed"))?;

            match &mut client.client_type {
                McpClientType::Grpc { client: grpc_client, .. } => {
                    let params = serde_json::json!({
                        "name": name_str,
                        "arguments": args_val
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
                            "name": name_str,
                            "arguments": args_val
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
        };

        match tokio::time::timeout(std::time::Duration::from_secs(30), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP tool call for '{}' timed out after 30s", name)),
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

// ── Lazy MCP tool wrapper ─────────────────────────────────────────────────────
// Holds server config + tool metadata but spawns the process only on first call.

/// Per-server lazy client, shared across all tools from the same MCP server.
static LAZY_MCP_CLIENTS: OnceLock<Mutex<std::collections::HashMap<String, McpClient>>> =
    OnceLock::new();

fn lazy_client_registry() -> &'static Mutex<std::collections::HashMap<String, McpClient>> {
    LAZY_MCP_CLIENTS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

pub struct LazyMcpToolWrapper {
    /// Human-readable MCP server name (e.g. "browser", "spreadsheet")
    pub server_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub name: String,
    pub description: String,
    pub parameters: Value,
    /// Whether this server was flagged as special (e.g. memory → set global client)
    pub is_memory_server: bool,
}

#[async_trait::async_trait]
impl Tool for LazyMcpToolWrapper {
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { &self.description }
    fn parameters(&self) -> Value { self.parameters.clone() }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let cache_key = format!("{}:{}", self.command, self.args.join(" "));

        // Fast path: already connected
        {
            let lock = lazy_client_registry().lock().await;
            if let Some(client) = lock.get(&cache_key) {
                match client.call_tool(&self.name, arguments).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        tracing::warn!("MCP tool call failed (server may have crashed), reconnecting: {}", e);
                        drop(lock);
                        let mut lock = lazy_client_registry().lock().await;
                        lock.remove(&cache_key);
                        if self.is_memory_server {
                            clear_memory_mcp_client();
                        }
                    }
                }
            }
        }

        // Slow path: first call or reconnection — spawn the MCP process
        tracing::info!(
            "Lazy-connecting MCP server '{}' for tool call: {}",
            self.server_name, self.name
        );

        let client = McpClient::spawn(&self.command, &self.args).await
            .map_err(|e| anyhow!(
                "Failed to connect MCP server '{}' on demand: {e}", self.server_name
            ))?;

        if self.is_memory_server {
            set_memory_mcp_client(client.clone());
        }

        {
            let mut lock = lazy_client_registry().lock().await;
            lock.insert(cache_key, client.clone());
        }

        client.call_tool(&self.name, arguments).await
    }
}


// -------------------- gRPC MCP BRIDGE IMPLEMENTATION --------------------

static NEXT_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(50060);

fn find_free_port() -> u16 {
    let start_port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if start_port > 65000 {
        NEXT_PORT.store(50060, std::sync::atomic::Ordering::Relaxed);
    }

    let mut port = start_port;
    for _ in 0..100 {
        if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
            return port;
        }
        port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    tracing::warn!("Could not find free port after 100 attempts, using last port {}", port);
    port
}

pub struct McpBridgeService {
    writer: Arc<Mutex<tokio::process::ChildStdin>>,
    senders: Arc<Mutex<std::collections::HashMap<i64, tokio::sync::oneshot::Sender<Value>>>>,
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
        
        if !req.has_id {
            let mut writer_lock = self.writer.lock().await;
            writer_lock.write_all(req_str.as_bytes()).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
            writer_lock.flush().await.map_err(|e| tonic::Status::internal(e.to_string()))?;
            return Ok(tonic::Response::new(mcp_grpc::McpResponse {
                result_json: String::new(),
                error_json: String::new(),
                id: 0,
            }));
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut senders_lock = self.senders.lock().await;
            senders_lock.insert(req.id, tx);
        }

        {
            let mut writer_lock = self.writer.lock().await;
            if let Err(e) = writer_lock.write_all(req_str.as_bytes()).await {
                let mut senders_lock = self.senders.lock().await;
                senders_lock.remove(&req.id);
                return Err(tonic::Status::internal(e.to_string()));
            }
            if let Err(e) = writer_lock.flush().await {
                let mut senders_lock = self.senders.lock().await;
                senders_lock.remove(&req.id);
                return Err(tonic::Status::internal(e.to_string()));
            }
        }

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(resp_val)) => {
                let result_json = resp_val.get("result").map(|v| v.to_string()).unwrap_or_default();
                let error_json = resp_val.get("error").map(|v| v.to_string()).unwrap_or_default();
                let id = resp_val.get("id").and_then(|v| v.as_i64()).unwrap_or(req.id);
                
                Ok(tonic::Response::new(mcp_grpc::McpResponse {
                    result_json,
                    error_json,
                    id,
                }))
            }
            Ok(Err(_)) => {
                Err(tonic::Status::aborted("Background stdout reader task exited or dropped connection"))
            }
            Err(_) => {
                let mut senders_lock = self.senders.lock().await;
                senders_lock.remove(&req.id);
                Err(tonic::Status::deadline_exceeded("MCP tool call timed out waiting for response"))
            }
        }
    }
}

pub async fn run_mcp_bridge(
    port: u16, 
    command: &str, 
    args: &[String], 
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>
) -> Result<()> {
    tracing::info!("Launching target MCP server: {} {:?}", command, args);
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to open child stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("Failed to open child stdout"))?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("Failed to open child stderr"))?;

    let cmd_log = command.to_string();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        while let Ok(n) = reader.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                tracing::warn!("[MCP Stderr - {}] {}", cmd_log, trimmed);
            }
            line.clear();
        }
    });

    let senders: Arc<Mutex<std::collections::HashMap<i64, tokio::sync::oneshot::Sender<Value>>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));

    let senders_clone = senders.clone();
    let reader_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    tracing::info!("Child stdout EOF reached");
                    break;
                }
                Ok(_) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(resp_val) = serde_json::from_str::<serde_json::Value>(&line) {
                        if resp_val.get("jsonrpc").is_some() {
                            if let Some(id_val) = resp_val.get("id") {
                                if let Some(id) = id_val.as_i64() {
                                    let mut senders_lock = senders_clone.lock().await;
                                    if let Some(tx) = senders_lock.remove(&id) {
                                        let _ = tx.send(resp_val);
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                    // Filter out non-JSON-RPC stdout messages
                    tracing::warn!("Filtered non-JSON-RPC stdio output from bridge: {}", line.trim());
                }
                Err(e) => {
                    tracing::error!("Error reading child stdout: {:?}", e);
                    break;
                }
            }
        }
    });

    let service = McpBridgeService {
        writer: Arc::new(Mutex::new(stdin)),
        senders,
    };

    let addr = format!("127.0.0.1:{}", port).parse()?;
    tracing::info!("gRPC MCP Bridge listening on {}", addr);

    let server_fut = tonic::transport::Server::builder()
        .add_service(mcp_grpc::mcp_service_server::McpServiceServer::new(service))
        .serve(addr);

    tokio::select! {
        res = server_fut => {
            res?;
        }
        _ = &mut shutdown_rx => {
            tracing::info!("gRPC MCP Bridge shutdown signal received for {}", command);
        }
    }

    reader_handle.abort();
    Ok(())
}
