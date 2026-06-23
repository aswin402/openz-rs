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

/// Cache of spawned MCP clients, keyed by "command:arg1 arg2 ...".
/// Shared across McpClient::spawn() and LazyMcpToolWrapper.
static SPAWNED_MCP_CLIENTS: OnceLock<Mutex<std::collections::HashMap<String, McpClient>>> =
    OnceLock::new();

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

pub async fn terminate_all_mcp_clients() {
    let cell = SPAWNED_MCP_CLIENTS.get();
    if let Some(mutex) = cell {
        let mut lock = mutex.lock().await;
        for (_key, client) in lock.drain() {
            let mut client_lock = client.0.lock().await;
            if let Some(inner) = client_lock.take() {
                drop(inner);
            }
        }
    }
    clear_memory_mcp_client();
}

pub mod mcp_grpc {
    tonic::include_proto!("mcp");
}

pub enum McpClientType {
    Grpc {
        child: Option<Child>,
        client: mcp_grpc::mcp_service_client::McpServiceClient<tonic::transport::Channel>,
    }
}

pub struct McpClientInner {
    client_type: McpClientType,
    next_id: usize,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    command: String,
    args: Vec<String>,
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
        }
    }
}

#[derive(Clone)]
pub struct McpClient(Arc<Mutex<Option<McpClientInner>>>);

impl McpClient {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self> {
        let cache_key = format!("{}:{}", command, args.join(" "));
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
                    command: cmd.clone(),
                    args: args_vec.clone(),
                };

                return Ok(McpClient(Arc::new(Mutex::new(Some(client)))));
            }

            // Automatic in-process gRPC bridge for all stdio servers to avoid stdio pollution
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            let (bridge_port, port_guard) = find_free_port()?;
            let cmd_string = cmd.clone();
            let cmd_args = args_vec.clone();

            tokio::spawn(async move {
                if let Err(e) = run_mcp_bridge(bridge_port, port_guard, &cmd_string, &cmd_args, shutdown_rx).await {
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
                command: cmd.clone(),
                args: args_vec.clone(),
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
            }
        };

        match tokio::time::timeout(std::time::Duration::from_secs(30), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP tool call for '{}' timed out after 30s", name)),
        }
    }

    pub async fn ping(&self) -> Result<()> {
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
                    Ok(())
                }
            }
        };

        match tokio::time::timeout(std::time::Duration::from_millis(2000), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP ping timed out")),
        }
    }

    /// Remove a stale entry from the spawn cache so the next spawn creates a fresh connection.
    pub async fn invalidate(command: &str, args: &[String]) {
        let cache_key = format!("{}:{}", command, args.join(" "));
        if let Some(cell) = SPAWNED_MCP_CLIENTS.get() {
            let mut lock = cell.lock().await;
            lock.remove(&cache_key);
        }
    }
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
        // Delegate to McpClient::spawn() which has its own internal cache
        // (SPAWNED_MCP_CLIENTS). Fast path returns existing client; slow path
        // spawns a fresh process and caches it.
        let client = McpClient::spawn(&self.command, &self.args).await
            .map_err(|e| anyhow!(
                "Failed to connect MCP server '{}' on demand: {e}", self.server_name
            ))?;

        if self.is_memory_server {
            set_memory_mcp_client(client.clone());
        }

        match client.call_tool(&self.name, arguments).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::warn!("MCP tool call failed (server may have crashed), re-spawning: {}", e);
                McpClient::invalidate(&self.command, &self.args).await;
                if self.is_memory_server {
                    clear_memory_mcp_client();
                }
                let client = McpClient::spawn(&self.command, &self.args).await
                    .map_err(|e2| anyhow!(
                        "Failed to reconnect MCP server '{}' after crash: {e2}", self.server_name
                    ))?;
                if self.is_memory_server {
                    set_memory_mcp_client(client.clone());
                }
                client.call_tool(&self.name, arguments).await
            }
        }
    }
}

static HEALTH_CHECK_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn start_mcp_health_checks() {
    if HEALTH_CHECK_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return; // Already started
    }

    tokio::spawn(async move {
        let mut unhealthy_servers: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
                _ = shutdown_rx.changed() => {
                    break;
                }
            }

            let cell = if let Some(cell) = SPAWNED_MCP_CLIENTS.get() {
                cell
            } else {
                continue;
            };

            // Snapshot cache keys and their details
            let mut clients_info = Vec::new();
            {
                let lock = cell.lock().await;
                for (cache_key, client) in lock.iter() {
                    if let Ok(guard) = client.0.try_lock() {
                        if let Some(inner) = guard.as_ref() {
                            clients_info.push((
                                cache_key.clone(),
                                inner.command.clone(),
                                inner.args.clone(),
                                client.clone(),
                            ));
                        }
                    }
                }
            }

            for (cache_key, command, args, client) in clients_info {
                let server_name = {
                    let parts: Vec<&str> = cache_key.splitn(2, ':').collect();
                    parts.first().copied().unwrap_or(&command).to_string()
                };

                match client.ping().await {
                    Ok(_) => {
                        if unhealthy_servers.remove(&cache_key) {
                            let recovery_msg = format!("⚡ MCP Server reconnected: {}", server_name);
                            tracing::info!("{}", recovery_msg);
                            crate::channels::send_notification(&recovery_msg);
                        }
                    }
                    Err(e) => {
                        let is_already_unhealthy = unhealthy_servers.contains(&cache_key);
                        if !is_already_unhealthy {
                            unhealthy_servers.insert(cache_key.clone());
                            let warn_msg = format!(
                                "⚠️ MCP Server '{}' is unresponsive ({}). Attempting auto-reconnect...",
                                server_name, e
                            );
                            tracing::warn!("{}", warn_msg);
                            crate::channels::send_notification(&warn_msg);
                        }

                        // Invalidate and try to spawn
                        McpClient::invalidate(&command, &args).await;
                        match McpClient::spawn(&command, &args).await {
                            Ok(_) => {
                                unhealthy_servers.remove(&cache_key);
                                let success_msg = format!("⚡ MCP Server '{}' successfully reconnected!", server_name);
                                tracing::info!("{}", success_msg);
                                crate::channels::send_notification(&success_msg);
                            }
                            Err(spawn_err) => {
                                let fail_msg = format!(
                                    "❌ MCP Server '{}' reconnect failed: {}",
                                    server_name, spawn_err
                                );
                                tracing::error!("{}", fail_msg);
                                crate::channels::send_notification(&fail_msg);
                            }
                        }
                    }
                }
            }
        }
    });
}


// -------------------- gRPC MCP BRIDGE IMPLEMENTATION --------------------

static NEXT_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(50060);

fn find_free_port() -> Result<(u16, std::net::TcpListener)> {
    let start_port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if start_port > 65000 {
        NEXT_PORT.store(50060, std::sync::atomic::Ordering::Relaxed);
    }

    let mut port = start_port;
    for _ in 0..100 {
        if let Ok(listener) = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
            return Ok((port, listener));
        }
        port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    tracing::warn!("Could not find free port after 100 attempts, using last port {}", port);
    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .map_err(|e| anyhow::anyhow!("Failed to bind fallback port {}: {}", port, e))?;
    Ok((port, listener))
}

struct SenderGuard {
    senders: Arc<Mutex<std::collections::HashMap<i64, tokio::sync::oneshot::Sender<Value>>>>,
    id: i64,
}

impl Drop for SenderGuard {
    fn drop(&mut self) {
        let senders = self.senders.clone();
        let id = self.id;
        tokio::spawn(async move {
            let mut senders_lock = senders.lock().await;
            senders_lock.remove(&id);
        });
    }
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
        let _guard = SenderGuard {
            senders: self.senders.clone(),
            id: req.id,
        };

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
    port_guard: std::net::TcpListener,
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
    let stderr_handle = tokio::spawn(async move {
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

    // Drop the port guard right before serving to minimize the bind window.
    // The guard held the port open since find_free_port(); dropping now means
    // tonic::Server::serve() binds immediately after, shrinking the TOCTOU gap
    // from ~100ms (spawn latency) to <1µs (format/parse overhead).
    drop(port_guard);

    tracing::info!("gRPC MCP Bridge listening on {}", addr);

    let server_fut = tonic::transport::Server::builder()
        .add_service(mcp_grpc::mcp_service_server::McpServiceServer::new(service))
        .serve(addr);

    // Monitor child process exit so we can shut down the bridge if the child crashes
    let cmd_name = command.to_string();
    let child_exit = async move {
        let status = child.wait().await?;
        tracing::warn!("MCP child process '{}' exited with: {:?}", cmd_name, status);
        anyhow::Ok(())
    };

    let cmd_shutdown = command.to_string();
    tokio::select! {
        res = server_fut => {
            res?;
        }
        res = child_exit => {
            if let Err(e) = res {
                tracing::error!("MCP child process '{}' wait error: {}", command, e);
            }
        }
        _ = &mut shutdown_rx => {
            tracing::info!("gRPC MCP Bridge shutdown signal received for {}", cmd_shutdown);
        }
    }

    // Abort reader tasks — they're no longer needed.
    reader_handle.abort();
    stderr_handle.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_free_port_returns_bound_listener() {
        let (port, listener) = find_free_port().unwrap();
        assert!(port >= 50060, "port should be in dynamic range");
        assert!(port <= 65000, "port should be in dynamic range");
        // Listener should be alive — binding same port again should fail
        assert!(
            std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_err(),
            "port should still be held by the guard listener"
        );
        // Drop the listener, then binding should succeed
        drop(listener);
        assert!(
            std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok(),
            "port should be free after guard is dropped"
        );
    }

    #[test]
    fn test_find_free_port_sequential_ports_differ() {
        let (_p1, l1) = find_free_port().unwrap();
        let (p2, _l2) = find_free_port().unwrap();
        // Second call should advance to a different port
        assert_ne!(p2, 0);
        drop(l1);
    }

    #[tokio::test]
    async fn test_invalidate_nonexistent_key_does_not_panic() {
        // Should not panic when cache is empty
        McpClient::invalidate("nonexistent", &[]).await;
    }

    #[tokio::test]
    async fn test_invalidate_after_spawn_removes_entry() {
        // Create a non-existent command — spawn should fail, no entry cached
        let result = McpClient::spawn("this-command-does-not-exist-12345", &[]).await;
        assert!(result.is_err(), "spawn of nonexistent command should fail");

        // Invalidate on something that was never cached should not panic
        McpClient::invalidate("this-command-does-not-exist-12345", &[]).await;
    }

    #[tokio::test]
    async fn test_ping_on_closed_client_returns_error() {
        let client = McpClient(Arc::new(Mutex::new(None)));
        assert!(client.ping().await.is_err());
    }
}
