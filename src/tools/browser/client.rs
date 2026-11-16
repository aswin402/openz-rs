//! Async Unix-socket client and auto-spawner for OpenZ's native WebKit2GTK browser worker.

use super::webkit_ffi::probe_webkit_runtime;
use super::worker::{browser_sockets_dir, sanitize_session_name, session_socket_path};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct SessionLaunchSpec {
    pub session: String,
    pub gui: bool,
    pub profile: Option<String>,
    pub width: i32,
    pub height: i32,
    pub memory_limit_mb: Option<u32>,
}

impl Default for SessionLaunchSpec {
    fn default() -> Self {
        let cfg = crate::config::loader::load_config().unwrap_or_default();
        Self {
            session: "default".to_string(),
            gui: cfg.browser.default_mode.eq_ignore_ascii_case("gui"),
            profile: None,
            width: cfg.browser.default_width,
            height: cfg.browser.default_height,
            memory_limit_mb: Some(cfg.browser.memory_limit_mb),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotRequest {
    pub path: PathBuf,
    pub full_page: bool,
    pub transparent: bool,
    pub annotated: bool,
    pub selector: Option<String>,
}

/// Reusable async handle to an isolated native `openz browser --worker` session.
#[derive(Debug, Clone)]
pub struct BrowserSessionClient {
    socket_path: PathBuf,
    session_name: String,
    ephemeral_owned: bool,
}

impl BrowserSessionClient {
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn session_name(&self) -> &str {
        &self.session_name
    }

    /// Connect to an existing named session or spawn a fresh worker automatically.
    pub async fn ensure_session(spec: &SessionLaunchSpec) -> Result<Self> {
        let safe_session = sanitize_session_name(&spec.session);
        let socket_path = session_socket_path(&safe_session);
        let client = Self {
            socket_path: socket_path.clone(),
            session_name: safe_session.clone(),
            ephemeral_owned: false,
        };

        match client.ping().await {
            Ok(status_val) => {
                let current_mode = status_val
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("headless");
                let want_mode = match spec.gui {
                    true => "gui",
                    false => "headless",
                };
                match spec.gui && current_mode != want_mode {
                    true => {
                        let _ = client
                            .request(
                                json!({ "op": "mode", "mode": "gui" }),
                                Duration::from_secs(5),
                            )
                            .await;
                    }
                    false => {}
                }
                return Ok(client);
            }
            Err(_) => {}
        }

        spawn_worker_process(&socket_path, &safe_session, spec, false)?;
        wait_for_worker_ready(&client, Duration::from_secs(8)).await?;
        Ok(client)
    }

    /// Spawn a dedicated ephemeral headless worker (used by `browser_broker`, `generate_image`,
    /// `html_to_video`, `compile_template`, and `openmedia`) so concurrent renders never collide.
    pub async fn spawn_ephemeral_headless(width: i32, height: i32) -> Result<Self> {
        let short_id = uuid::Uuid::new_v4().simple().to_string();
        let session_name = format!("eph_{}", &short_id[..12]);
        let socket_path = session_socket_path(&session_name);
        let cfg = crate::config::loader::load_config().unwrap_or_default();
        let spec = SessionLaunchSpec {
            session: session_name.clone(),
            gui: false,
            profile: None,
            width,
            height,
            memory_limit_mb: Some(cfg.browser.memory_limit_mb),
        };
        spawn_worker_process(&socket_path, &session_name, &spec, true)?;
        let client = Self {
            socket_path,
            session_name,
            ephemeral_owned: true,
        };
        wait_for_worker_ready(&client, Duration::from_secs(8)).await?;
        Ok(client)
    }

    pub async fn ping(&self) -> Result<Value> {
        self.request(json!({ "op": "status" }), Duration::from_secs(2))
            .await
    }

    pub async fn request(&self, payload: Value, timeout: Duration) -> Result<Value> {
        #[cfg(unix)]
        {
            let stream = tokio::time::timeout(
                Duration::from_secs(2),
                tokio::net::UnixStream::connect(&self.socket_path),
            )
            .await
            .map_err(|_| anyhow!("Timed out connecting to browser socket {:?}", self.socket_path))?
            .map_err(|e| anyhow!("Browser socket {:?} unavailable: {}", self.socket_path, e))?;

            let (read_half, mut write_half) = stream.into_split();
            let mut encoded = serde_json::to_string(&payload)?;
            encoded.push('\n');
            write_half.write_all(encoded.as_bytes()).await?;
            write_half.flush().await?;

            let mut reader = BufReader::new(read_half);
            let mut reply_line = String::new();
            let read_res = tokio::time::timeout(timeout, reader.read_line(&mut reply_line))
                .await
                .map_err(|_| anyhow!("Timed out after {:?} waiting for browser worker", timeout))??;

            match read_res == 0 {
                true => return Err(anyhow!("Browser worker closed IPC stream unexpectedly")),
                false => {}
            }

            let parsed: Value = serde_json::from_str(reply_line.trim())?;
            let ok = parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            match ok {
                true => Ok(parsed),
                false => {
                    let err_msg = parsed
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown browser worker error");
                    Err(anyhow!("{err_msg}"))
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (payload, timeout);
            Err(anyhow!("Unix domain socket IPC requires a Unix host"))
        }
    }

    pub async fn navigate_url(
        &self,
        url: &str,
        timeout_ms: u64,
        settle_ms: u64,
    ) -> Result<Value> {
        let total_wait = Duration::from_millis(timeout_ms + settle_ms + 3_000);
        self.request(
            json!({
                "op": "navigate",
                "url": url,
                "timeout_ms": timeout_ms,
                "settle_ms": settle_ms,
            }),
            total_wait,
        )
        .await
    }

    pub async fn load_html(
        &self,
        html: &str,
        base_uri: Option<&str>,
        settle_ms: u64,
    ) -> Result<Value> {
        self.request(
            json!({
                "op": "navigate",
                "html": html,
                "base_uri": base_uri,
                "timeout_ms": 10_000,
                "settle_ms": settle_ms,
            }),
            Duration::from_secs(15),
        )
        .await
    }

    pub async fn resize(&self, width: i32, height: i32, scale: f64) -> Result<Value> {
        self.request(
            json!({
                "op": "resize",
                "width": width,
                "height": height,
                "scale": scale,
            }),
            Duration::from_secs(5),
        )
        .await
    }

    pub async fn set_mode(&self, gui: bool) -> Result<Value> {
        let mode = match gui {
            true => "gui",
            false => "headless",
        };
        self.request(json!({ "op": "mode", "mode": mode }), Duration::from_secs(5))
            .await
    }

    pub async fn web_agent(&self, action_payload: Value, timeout: Duration) -> Result<Value> {
        let res = self
            .request(
                json!({
                    "op": "web_agent",
                    "payload": action_payload,
                }),
                timeout,
            )
            .await?;
        Ok(res.get("data").cloned().unwrap_or(Value::Null))
    }

    pub async fn eval_js(&self, script: &str, timeout: Duration) -> Result<Value> {
        let timeout_ms = timeout.as_millis().min(38_000) as u64;
        self.web_agent(
            json!({
                "action": "eval_js",
                "script": script,
                "timeout_ms": timeout_ms,
            }),
            timeout + Duration::from_secs(2),
        )
        .await
    }

    pub async fn screenshot(&self, req: &ScreenshotRequest) -> Result<Value> {
        match req.annotated {
            true => {
                let _ = self
                    .web_agent(
                        json!({ "action": "inspect_overlay", "enabled": true }),
                        Duration::from_secs(4),
                    )
                    .await;
            }
            false => {}
        }

        let crop_val = match req.selector.as_deref() {
            Some(sel) => {
                let rect_res = self
                    .web_agent(
                        json!({ "action": "element_rect", "selector": sel }),
                        Duration::from_secs(4),
                    )
                    .await
                    .ok();
                rect_res
                    .filter(|v| v.get("found").and_then(|b| b.as_bool()) == Some(true))
                    .and_then(|v| v.get("rect").cloned())
            }
            None => None,
        };

        let mut cmd = json!({
            "op": "screenshot",
            "path": req.path.to_string_lossy(),
            "full_page": req.full_page,
            "transparent": req.transparent,
        });
        match crop_val {
            Some(rect) => cmd["crop"] = rect,
            None => {}
        }

        let snap_res = self.request(cmd, Duration::from_secs(20)).await;

        match req.annotated {
            true => {
                let _ = self
                    .web_agent(
                        json!({ "action": "inspect_overlay", "enabled": false }),
                        Duration::from_secs(3),
                    )
                    .await;
            }
            false => {}
        }

        snap_res
    }

    pub async fn save_pdf(&self, output_path: &Path) -> Result<Value> {
        self.request(
            json!({
                "op": "save_pdf",
                "path": output_path.to_string_lossy(),
            }),
            Duration::from_secs(25),
        )
        .await
    }

    pub async fn close(&self) -> Result<()> {
        let _ = self
            .request(json!({ "op": "stop" }), Duration::from_secs(3))
            .await;
        let _ = std::fs::remove_file(&self.socket_path);
        Ok(())
    }
}

impl Drop for BrowserSessionClient {
    fn drop(&mut self) {
        match self.ephemeral_owned {
            true => {
                let sock = self.socket_path.clone();
                #[cfg(unix)]
                {
                    if_connected_send_stop_sync(&sock);
                }
            }
            false => {}
        }
    }
}

#[cfg(unix)]
fn if_connected_send_stop_sync(socket_path: &Path) {
    use std::io::Write;
    match std::os::unix::net::UnixStream::connect(socket_path) {
        Ok(mut stream) => {
            let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
            let _ = stream.write_all(b"{\"op\":\"stop\"}\n");
            let _ = stream.flush();
        }
        Err(_) => {}
    }
    let _ = std::fs::remove_file(socket_path);
}

fn spawn_worker_process(
    socket_path: &Path,
    session_name: &str,
    spec: &SessionLaunchSpec,
    parent_death_sigterm: bool,
) -> Result<()> {
    let probe = probe_webkit_runtime();
    match probe.available {
        true => {}
        false => {
            let detail = probe
                .error
                .unwrap_or_else(|| "Native WebKit2GTK runtime unavailable".to_string());
            return Err(anyhow!("{detail}"));
        }
    }

    match socket_path.parent() {
        Some(parent) => std::fs::create_dir_all(parent)?,
        None => {}
    }

    let exe = resolve_openz_worker_executable()?;
    let cfg = crate::config::loader::load_config().unwrap_or_default();
    let mem_mb = spec
        .memory_limit_mb
        .unwrap_or(cfg.browser.memory_limit_mb)
        .clamp(48, 2048);

    let mut cmd = Command::new(&exe);
    cmd.arg("browser")
        .arg("--worker")
        .arg("--session")
        .arg(session_name)
        .arg("--socket")
        .arg(socket_path)
        .arg("--width")
        .arg(spec.width.clamp(120, 7680).to_string())
        .arg("--height")
        .arg(spec.height.clamp(120, 7680).to_string())
        .arg("--memory-mb")
        .arg(mem_mb.to_string())
        .arg("--idle-timeout")
        .arg(cfg.browser.idle_timeout_secs.to_string());

    match spec.gui {
        true => {
            cmd.arg("--gui");
        }
        false => {
            cmd.arg("--headless");
        }
    }

    match parent_death_sigterm || !spec.gui {
        true => {
            cmd.arg("--parent-death-sigterm");
        }
        false => {}
    }

    match spec.profile.as_deref() {
        Some(prof) => {
            cmd.arg("--profile").arg(prof);
        }
        None => {}
    }

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd.spawn().map_err(|e| {
        anyhow!(
            "Failed to spawn native OpenZ browser worker ({:?}): {}",
            exe,
            e
        )
    })?;

    let kind = match spec.gui {
        true => "browser_gui",
        false => "browser",
    };
    crate::shutdown::register_child_group_with_metadata(
        child,
        format!("openz browser --worker --session {session_name}"),
        kind,
    );
    Ok(())
}

fn resolve_openz_worker_executable() -> Result<PathBuf> {
    let current = std::env::current_exe()?;
    // Under `cargo test`, `current_exe()` is the test harness binary inside `target/debug/deps/...`.
    // Prefer a sibling compiled `openz` binary or `~/.cargo/bin/openz` when running inside unit tests.
    let file_stem = current
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let is_openz_bin = file_stem == "openz" || file_stem == "openz.exe";
    match is_openz_bin {
        true => Ok(current),
        false => {
            let sibling = current
                .parent()
                .and_then(|deps| deps.parent())
                .map(|dir| dir.join("openz"))
                .filter(|p| p.exists());
            match sibling {
                Some(bin) => Ok(bin),
                None => {
                    let home_cargo = dirs::home_dir()
                        .map(|h| h.join(".cargo/bin/openz"))
                        .filter(|p| p.exists());
                    match home_cargo {
                        Some(bin) => Ok(bin),
                        None => Ok(current),
                    }
                }
            }
        }
    }
}

async fn wait_for_worker_ready(client: &BrowserSessionClient, max_wait: Duration) -> Result<()> {
    let started = std::time::Instant::now();
    while started.elapsed() < max_wait {
        match client.ping().await {
            Ok(_) => return Ok(()),
            Err(_) => sleep(Duration::from_millis(35)).await,
        }
    }
    Err(anyhow!(
        "Timed out waiting for native OpenZ browser worker socket {:?}",
        client.socket_path()
    ))
}

/// Enumerate all live native browser sessions by probing active sockets in `browser_sockets_dir()`.
pub async fn list_active_browser_sessions() -> Vec<Value> {
    let dir = browser_sockets_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut active = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_sock = path.extension().and_then(|e| e.to_str()) == Some("sock");
        match is_sock {
            false => continue,
            true => {}
        }
        let session_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("default")
            .to_string();
        let client = BrowserSessionClient {
            socket_path: path.clone(),
            session_name,
            ephemeral_owned: false,
        };
        match client.ping().await {
            Ok(info) => active.push(info),
            Err(_) => {
                // Clean up stale socket file left by an exited worker
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    active
}

/// High-level helper: render HTML/URL into a PNG file using an ephemeral native headless worker.
#[allow(clippy::too_many_arguments)]
pub async fn render_target_to_png(
    target_url: Option<&str>,
    raw_html: Option<&str>,
    custom_css: Option<&str>,
    width: i32,
    height: i32,
    scale: f64,
    settle_ms: u64,
    selector: Option<&str>,
    transparent: bool,
    output_path: &Path,
) -> Result<Value> {
    let client = BrowserSessionClient::spawn_ephemeral_headless(width, height).await?;
    match (scale - 1.0).abs() > f64::EPSILON {
        true => {
            let _ = client.resize(width, height, scale).await;
        }
        false => {}
    }

    match (target_url, raw_html) {
        (Some(url), _) => {
            client.navigate_url(url, 12_000, settle_ms).await?;
        }
        (None, Some(html)) => {
            client.load_html(html, None, settle_ms).await?;
        }
        (None, None) => {
            return Err(anyhow!("render_target_to_png requires target_url or raw_html"));
        }
    }

    match custom_css.filter(|s| !s.trim().is_empty()) {
        Some(css) => {
            let _ = client
                .web_agent(
                    json!({ "action": "inject_css", "css": css }),
                    Duration::from_secs(5),
                )
                .await;
        }
        None => {}
    }

    let snap = client
        .screenshot(&ScreenshotRequest {
            path: output_path.to_path_buf(),
            full_page: false,
            transparent,
            annotated: false,
            selector: selector.map(ToString::to_string),
        })
        .await?;
    let _ = client.close().await;
    Ok(snap)
}

/// High-level helper: render HTML or URL directly to a multi-page Cairo PDF document.
pub async fn export_html_or_url_to_pdf(
    target_url: Option<&str>,
    raw_html: Option<&str>,
    base_uri: Option<&str>,
    output_path: &Path,
) -> Result<Value> {
    let client = BrowserSessionClient::spawn_ephemeral_headless(1240, 1754).await?;
    match (target_url, raw_html) {
        (Some(url), _) => {
            client.navigate_url(url, 15_000, 300).await?;
        }
        (None, Some(html)) => {
            client.load_html(html, base_uri, 300).await?;
        }
        (None, None) => {
            return Err(anyhow!(
                "export_html_or_url_to_pdf requires target_url or raw_html"
            ));
        }
    }
    let pdf_res = client.save_pdf(output_path).await?;
    let _ = client.close().await;
    Ok(pdf_res)
}
