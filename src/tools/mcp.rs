use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

static LOADED_MCPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static FAILED_MCPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static TOTAL_MCPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static MCP_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_mcp_status(loaded: u32, failed: u32) {
    LOADED_MCPS.store(loaded, std::sync::atomic::Ordering::Relaxed);
    FAILED_MCPS.store(failed, std::sync::atomic::Ordering::Relaxed);
}

pub fn init_mcp_progress(total: u32) {
    TOTAL_MCPS.store(total, std::sync::atomic::Ordering::Relaxed);
    LOADED_MCPS.store(0, std::sync::atomic::Ordering::Relaxed);
    FAILED_MCPS.store(0, std::sync::atomic::Ordering::Relaxed);
    MCP_DONE.store(false, std::sync::atomic::Ordering::Relaxed);
}

pub fn increment_mcp_loaded() {
    LOADED_MCPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn increment_mcp_failed() {
    FAILED_MCPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn set_mcp_done() {
    MCP_DONE.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_mcp_done() -> bool {
    MCP_DONE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn get_mcp_stats() -> (u32, u32, u32) {
    (
        LOADED_MCPS.load(std::sync::atomic::Ordering::Relaxed),
        FAILED_MCPS.load(std::sync::atomic::Ordering::Relaxed),
        TOTAL_MCPS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Cache of spawned MCP clients, keyed by "command:arg1 arg2 ...".
/// Shared across McpClient::spawn() and LazyMcpToolWrapper.
static SPAWNED_MCP_CLIENTS: OnceLock<Mutex<std::collections::HashMap<String, McpClient>>> =
    OnceLock::new();

pub async fn terminate_all_mcp_clients() {
    match SPAWNED_MCP_CLIENTS.get() {
        Some(mutex) => {
            let mut lock = mutex.lock().await;
            for (_key, client) in lock.drain() {
                let mut client_lock = client.0.lock().await;
                drop(client_lock.take());
            }
        }
        None => {}
    }
}

pub mod mcp_grpc {
    tonic::include_proto!("mcp");
}

pub enum McpClientType {
    Grpc {
        child: Option<Child>,
        client: mcp_grpc::mcp_service_client::McpServiceClient<tonic::transport::Channel>,
    },
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
        match self.shutdown_tx.take() {
            Some(tx) => {
                let _ = tx.send(());
            }
            None => {}
        }
        match &mut self.client_type {
            McpClientType::Grpc { ref mut child, .. } => match child {
                Some(ref mut c) => {
                    let _ = c.start_kill();
                }
                None => {}
            },
        }
    }
}

/// Returns the live count of tools advertised by a previously spawned MCP server,
/// keyed by the same `command:args` cache key used during `McpClient::spawn`.
/// Returns 0 when the server has not been spawned (e.g. disabled or not yet started).
pub async fn spawned_tools_count(command: &str, args: &[String]) -> usize {
    let cache_key = format!("{}:{}", command, args.join(" "));
    match SPAWNED_MCP_CLIENTS.get() {
        None => 0,
        Some(mutex) => {
            let lock = mutex.lock().await;
            match lock.get(&cache_key) {
                None => 0,
                Some(client) => match client.list_tools().await {
                    Ok(tools) => tools.len(),
                    Err(_) => 0,
                },
            }
        }
    }
}

async fn connect_and_initialize_grpc_client(
    port: u16,
    target_label: &str,
) -> Result<mcp_grpc::mcp_service_client::McpServiceClient<tonic::transport::Channel>> {
    let mut channel = None;
    for i in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        match tonic::transport::Channel::from_shared(format!("http://127.0.0.1:{}", port))?
            .connect_timeout(std::time::Duration::from_secs(1))
            .connect()
            .await
        {
            Ok(ch) => {
                channel = Some(ch);
                break;
            }
            Err(e) => match i == 19 {
                true => {
                    return Err(anyhow!(
                        "Failed to connect to {} after 3s: {}",
                        target_label,
                        e
                    ));
                }
                false => {}
            },
        }
    }
    let channel =
        channel.ok_or_else(|| anyhow!("Failed to connect to {} after 3s", target_label))?;
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

    Ok(grpc_client)
}

fn extract_grpc_port(args_vec: &[String]) -> Option<u16> {
    let pos = args_vec.iter().position(|a| a == "--grpc")?;
    let port_str = args_vec.get(pos + 1)?;
    port_str.parse::<u16>().ok()
}

#[derive(Clone)]
pub struct McpClient(Arc<Mutex<Option<McpClientInner>>>);

impl McpClient {
    pub async fn spawn(command: &str, args: &[String]) -> Result<Self> {
        let cache_key = format!("{}:{}", command, args.join(" "));
        let cell = SPAWNED_MCP_CLIENTS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));

        {
            let lock = cell.lock().await;
            match lock.get(&cache_key) {
                Some(client) => return Ok(client.clone()),
                None => {}
            }
        }

        let cmd = command.to_string();
        let args_vec = args.to_vec();

        let fut = async move {
            match extract_grpc_port(&args_vec) {
                Some(port) => {
                    let child = Command::new(&cmd)
                        .args(&args_vec)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .kill_on_drop(true)
                        .spawn()?;

                    let label = format!("gRPC MCP server at 127.0.0.1:{}", port);
                    let grpc_client = connect_and_initialize_grpc_client(port, &label).await?;

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

                    Ok(McpClient(Arc::new(Mutex::new(Some(client)))))
                }
                None => {
                    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                    let (bridge_port, port_guard) = find_free_port()?;
                    let cmd_string = cmd.clone();
                    let cmd_args = args_vec.clone();

                    tokio::spawn(async move {
                        match run_mcp_bridge(
                            bridge_port,
                            port_guard,
                            &cmd_string,
                            &cmd_args,
                            shutdown_rx,
                        )
                        .await
                        {
                            Ok(()) => {}
                            Err(e) => {
                                tracing::error!(
                                    "In-process gRPC MCP bridge failed for {}: {:?}",
                                    cmd_string,
                                    e
                                );
                            }
                        }
                    });

                    let label = format!("local gRPC bridge for {}", cmd);
                    let grpc_client =
                        connect_and_initialize_grpc_client(bridge_port, &label).await?;

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
                }
            }
        };

        let result = match tokio::time::timeout(std::time::Duration::from_secs(15), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP spawn for {} timed out after 15s", command)),
        };

        match result {
            Ok(client) => {
                let mut lock = cell.lock().await;
                lock.insert(cache_key, client.clone());
                Ok(client)
            }
            Err(e) => Err(e),
        }
    }

    async fn send_grpc_request(
        &self,
        method: &str,
        params_json: String,
        err_prefix: &str,
    ) -> Result<String> {
        let mut lock = self.0.lock().await;
        let client = lock.as_mut().ok_or_else(|| anyhow!("Client closed"))?;

        match &mut client.client_type {
            McpClientType::Grpc {
                client: grpc_client,
                ..
            } => {
                let req = mcp_grpc::McpRequest {
                    method: method.to_string(),
                    params_json,
                    id: client.next_id as i64,
                    has_id: true,
                };
                client.next_id += 1;

                let resp = grpc_client.call(req).await?.into_inner();
                match resp.error_json.is_empty() {
                    true => Ok(resp.result_json),
                    false => Err(anyhow!("{}: {}", err_prefix, resp.error_json)),
                }
            }
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<Value>> {
        let self_clone = self.clone();
        let fut = async move {
            let result_json = self_clone
                .send_grpc_request("tools/list", "{}".to_string(), "MCP error")
                .await?;
            let result_val: Value = serde_json::from_str(&result_json)?;
            let tools = result_val
                .get("tools")
                .and_then(|t| t.as_array())
                .ok_or_else(|| anyhow!("Invalid tools/list response"))?;
            Ok(tools.clone())
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
            let params = serde_json::json!({
                "name": name_str,
                "arguments": args_val
            });
            let result_json = self_clone
                .send_grpc_request(
                    "tools/call",
                    serde_json::to_string(&params)?,
                    "MCP tool call error",
                )
                .await?;
            let result_val: Value = serde_json::from_str(&result_json)?;
            Ok(result_val)
        };

        match tokio::time::timeout(std::time::Duration::from_secs(30), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP tool call for '{}' timed out after 30s", name)),
        }
    }

    pub async fn ping(&self) -> Result<()> {
        let self_clone = self.clone();
        let fut = async move {
            let _ = self_clone
                .send_grpc_request("tools/list", "{}".to_string(), "MCP error")
                .await?;
            Ok(())
        };

        match tokio::time::timeout(std::time::Duration::from_millis(2000), fut).await {
            Ok(res) => res,
            Err(_) => Err(anyhow!("MCP ping timed out")),
        }
    }

    /// Remove a stale entry from the spawn cache so the next spawn creates a fresh connection.
    pub async fn invalidate(command: &str, args: &[String]) {
        let cache_key = format!("{}:{}", command, args.join(" "));
        match SPAWNED_MCP_CLIENTS.get() {
            Some(cell) => {
                let mut lock = cell.lock().await;
                lock.remove(&cache_key);
            }
            None => {}
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
}

#[async_trait::async_trait]
impl Tool for LazyMcpToolWrapper {
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
        let client = McpClient::spawn(&self.command, &self.args)
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to connect MCP server '{}' on demand: {e}",
                    self.server_name
                )
            })?;

        match client.call_tool(&self.name, arguments).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::warn!(
                    "MCP tool call failed (server may have crashed), re-spawning: {}",
                    e
                );
                McpClient::invalidate(&self.command, &self.args).await;
                let client = McpClient::spawn(&self.command, &self.args)
                    .await
                    .map_err(|e2| {
                        anyhow!(
                            "Failed to reconnect MCP server '{}' after crash: {e2}",
                            self.server_name
                        )
                    })?;
                client.call_tool(&self.name, arguments).await
            }
        }
    }
}

static HEALTH_CHECK_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn start_mcp_health_checks() {
    match HEALTH_CHECK_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        true => return,
        false => {}
    }

    tokio::spawn(async move {
        let mut unhealthy_servers: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        loop {
            match *shutdown_rx.borrow() {
                true => break,
                false => {}
            }

            tokio::select! {
                biased;
                _ = shutdown_rx.changed() => {
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
            }

            let cell = match SPAWNED_MCP_CLIENTS.get() {
                Some(cell) => cell,
                None => continue,
            };

            let mut clients_info = Vec::new();
            {
                let lock = cell.lock().await;
                for (cache_key, client) in lock.iter() {
                    match client.0.try_lock().ok().and_then(|g| {
                        g.as_ref().map(|inner| {
                            (
                                cache_key.clone(),
                                inner.command.clone(),
                                inner.args.clone(),
                                client.clone(),
                            )
                        })
                    }) {
                        Some(info) => clients_info.push(info),
                        None => {}
                    }
                }
            }

            for (cache_key, command, args, client) in clients_info {
                let server_name = {
                    let parts: Vec<&str> = cache_key.splitn(2, ':').collect();
                    parts.first().copied().unwrap_or(&command).to_string()
                };

                match client.ping().await {
                    Ok(_) => match unhealthy_servers.remove(&cache_key) {
                        true => {
                            let recovery_msg =
                                format!("⚡ MCP Server reconnected: {}", server_name);
                            tracing::info!("{}", recovery_msg);
                            crate::channels::send_notification(&recovery_msg);
                        }
                        false => {}
                    },
                    Err(e) => {
                        match unhealthy_servers.contains(&cache_key) {
                            false => {
                                unhealthy_servers.insert(cache_key.clone());
                                let warn_msg = format!(
                                    "⚠️ MCP Server '{}' is unresponsive ({}). Attempting auto-reconnect...",
                                    server_name, e
                                );
                                tracing::warn!("{}", warn_msg);
                                crate::channels::send_notification(&warn_msg);
                            }
                            true => {}
                        }

                        McpClient::invalidate(&command, &args).await;
                        match McpClient::spawn(&command, &args).await {
                            Ok(_) => {
                                unhealthy_servers.remove(&cache_key);
                                let success_msg = format!(
                                    "⚡ MCP Server '{}' successfully reconnected!",
                                    server_name
                                );
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
    match start_port > 65000 {
        true => NEXT_PORT.store(50060, std::sync::atomic::Ordering::Relaxed),
        false => {}
    }

    let mut port = start_port;
    for _ in 0..100 {
        match std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(listener) => return Ok((port, listener)),
            Err(_) => {
                port = NEXT_PORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
    tracing::warn!(
        "Could not find free port after 100 attempts, using last port {}",
        port
    );
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
    async fn call(
        &self,
        request: tonic::Request<mcp_grpc::McpRequest>,
    ) -> std::result::Result<tonic::Response<mcp_grpc::McpResponse>, tonic::Status> {
        let req = request.into_inner();
        let params_val = serde_json::from_str::<serde_json::Value>(&req.params_json)
            .unwrap_or(serde_json::Value::Null);

        let rpc_req = match req.has_id {
            false => serde_json::json!({
                "jsonrpc": "2.0",
                "method": req.method,
                "params": params_val
            }),
            true => serde_json::json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "method": req.method,
                "params": params_val
            }),
        };

        let req_str = format!(
            "{}\n",
            serde_json::to_string(&rpc_req)
                .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?
        );

        match req.has_id {
            false => {
                let mut writer_lock = self.writer.lock().await;
                writer_lock
                    .write_all(req_str.as_bytes())
                    .await
                    .map_err(|e| tonic::Status::internal(e.to_string()))?;
                writer_lock
                    .flush()
                    .await
                    .map_err(|e| tonic::Status::internal(e.to_string()))?;
                return Ok(tonic::Response::new(mcp_grpc::McpResponse {
                    result_json: String::new(),
                    error_json: String::new(),
                    id: 0,
                }));
            }
            true => {}
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
            let write_res = match writer_lock.write_all(req_str.as_bytes()).await {
                Ok(()) => writer_lock.flush().await,
                Err(e) => Err(e),
            };
            match write_res {
                Ok(()) => {}
                Err(e) => {
                    let mut senders_lock = self.senders.lock().await;
                    senders_lock.remove(&req.id);
                    return Err(tonic::Status::internal(e.to_string()));
                }
            }
        }

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(resp_val)) => {
                let result_json = resp_val
                    .get("result")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let error_json = resp_val
                    .get("error")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let id = resp_val
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(req.id);

                Ok(tonic::Response::new(mcp_grpc::McpResponse {
                    result_json,
                    error_json,
                    id,
                }))
            }
            Ok(Err(_)) => Err(tonic::Status::aborted(
                "Background stdout reader task exited or dropped connection",
            )),
            Err(_) => {
                let mut senders_lock = self.senders.lock().await;
                senders_lock.remove(&req.id);
                Err(tonic::Status::deadline_exceeded(
                    "MCP tool call timed out waiting for response",
                ))
            }
        }
    }
}

fn try_extract_jsonrpc_response(line: &str) -> Option<(i64, Value)> {
    let resp_val = serde_json::from_str::<Value>(line).ok()?;
    resp_val.get("jsonrpc")?;
    let id = resp_val.get("id").and_then(|v| v.as_i64())?;
    Some((id, resp_val))
}

pub async fn run_mcp_bridge(
    port: u16,
    port_guard: std::net::TcpListener,
    command: &str,
    args: &[String],
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    tracing::info!("Launching target MCP server: {} {:?}", command, args);
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Failed to open child stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to open child stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to open child stderr"))?;

    let cmd_log = command.to_string();
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let trimmed = line.trim();
                    match trimmed.is_empty() {
                        false => tracing::warn!("[MCP Stderr - {}] {}", cmd_log, trimmed),
                        true => {}
                    }
                }
            }
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
                Ok(_) => match line.trim().is_empty() {
                    true => continue,
                    false => match try_extract_jsonrpc_response(&line) {
                        Some((id, resp_val)) => {
                            let mut senders_lock = senders_clone.lock().await;
                            match senders_lock.remove(&id) {
                                Some(tx) => {
                                    let _ = tx.send(resp_val);
                                }
                                None => {}
                            }
                        }
                        None => {
                            tracing::warn!(
                                "Filtered non-JSON-RPC stdio output from bridge: {}",
                                line.trim()
                            );
                        }
                    },
                },
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

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse()?;

    port_guard.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(port_guard)?;
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    tracing::info!("gRPC MCP Bridge listening on {}", addr);

    let server_fut = tonic::transport::Server::builder()
        .add_service(mcp_grpc::mcp_service_server::McpServiceServer::new(service))
        .serve_with_incoming(incoming);

    let cmd_name = command.to_string();
    let child_exit = async move {
        let status = child.wait().await?;
        tracing::warn!("MCP child process '{}' exited with: {:?}", cmd_name, status);
        anyhow::Ok(())
    };

    let cmd_shutdown = command.to_string();
    tokio::select! {
        biased;
        _ = &mut shutdown_rx => {
            tracing::info!("gRPC MCP Bridge shutdown signal received for {}", cmd_shutdown);
        }
        res = server_fut => {
            res?;
        }
        res = child_exit => {
            match res {
                Ok(()) => {}
                Err(e) => tracing::error!("MCP child process '{}' wait error: {}", command, e),
            }
        }
    }

    reader_handle.abort();
    stderr_handle.abort();
    Ok(())
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
