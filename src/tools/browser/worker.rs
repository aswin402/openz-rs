//! Isolated Native GTK3 + WebKit2GTK Browser Worker & OpenZ GUI Browser.
//!
//! Runs on the true OS main thread (before Tokio spawns worker threads) so allocator
//! and JavaScriptCore RAM limits take effect cleanly and GTK owns the main event loop.
//! Supports both zero-popup `GtkOffscreenWindow` headless execution and a sleek dark-themed
//! OpenZ GUI window, with live headless <-> GUI reparenting over a Unix domain socket IPC.

use super::webkit_ffi::detect_display_environment;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const PAGE_CONTROLLER_JS: &str = include_str!("page_controller.js");

const OPENZ_GTK_CSS: &str = r#"
window, headerbar {
  background-color: #0b0f19;
  color: #f8fafc;
}
headerbar {
  border-bottom: 1px solid #1e293b;
  padding: 4px 8px;
}
button {
  background-image: none;
  background-color: #161e2e;
  color: #e2e8f0;
  border: 1px solid #334155;
  border-radius: 6px;
  padding: 4px 10px;
}
button:hover {
  border-color: #ff4500;
  color: #ffffff;
}
entry {
  background-image: none;
  background-color: #0f172a;
  color: #f8fafc;
  border: 1px solid #334155;
  border-radius: 6px;
  padding: 4px 10px;
  caret-color: #ff4500;
}
entry:focus {
  border-color: #ff4500;
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserWorkerOptions {
    pub worker_mode: bool,
    pub gui: bool,
    pub socket_path: PathBuf,
    pub session_name: String,
    pub initial_url: Option<String>,
    pub profile: Option<String>,
    pub width: i32,
    pub height: i32,
    pub memory_limit_mb: u32,
    pub idle_timeout_secs: u64,
    pub parent_death_sigterm: bool,
    pub one_shot_screenshot: Option<PathBuf>,
    pub one_shot_pdf: Option<PathBuf>,
}

pub fn browser_sockets_dir() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    let uid = current_uid();
    base.join(format!("openz-browser-{uid}"))
}

fn current_uid() -> u32 {
    #[cfg(unix)]
    {
        unsafe { libc::getuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

pub fn sanitize_session_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|ch| match ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            true => ch,
            false => '_',
        })
        .take(48)
        .collect();
    match cleaned.is_empty() {
        true => "default".to_string(),
        false => cleaned,
    }
}

pub fn session_socket_path(session_name: &str) -> PathBuf {
    let safe = sanitize_session_name(session_name);
    browser_sockets_dir().join(format!("{safe}.sock"))
}

pub fn read_process_rss_kb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        status.lines().find_map(|line| {
            let rest = line.strip_prefix("VmRSS:")?;
            rest.split_whitespace().next()?.parse::<u64>().ok()
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Apply single-threaded allocator, JavaScriptCore, and display environment settings
/// BEFORE GTK/WebKit or any worker threads spawn.
pub fn tune_browser_process_env(memory_limit_mb: u32) {
    let clamped_mb = memory_limit_mb.clamp(48, 2048);
    let jsc_bytes = (clamped_mb as u64) * 1024 * 1024;
    set_env_default("MALLOC_ARENA_MAX", "2");
    set_env_default("JavaScriptCoreUseBMalloc", "0");
    set_env_default("JSC_forceRAMSize", &jsc_bytes.to_string());
    set_env_default("JSC_useFTLJIT", "0");
    set_env_default("JSC_useOMGJIT", "0");
    set_env_default("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    set_env_default("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    set_env_default("NO_AT_BRIDGE", "1");
}

fn set_env_default(key: &str, value: &str) {
    let exists = std::env::var_os(key).is_some();
    match exists {
        true => {}
        false => std::env::set_var(key, value),
    }
}

pub fn parse_size_spec(spec: &str) -> Option<(i32, i32)> {
    let lower = spec.trim().to_ascii_lowercase();
    match lower.as_str() {
        "desktop" => return Some((1280, 800)),
        "hd" | "1080p" => return Some((1920, 1080)),
        "tablet" | "ipad" => return Some((768, 1024)),
        "mobile" | "phone" => return Some((412, 915)),
        _ => {}
    }
    let (w_str, h_str) = lower.split_once('x').or_else(|| lower.split_once(','))?;
    let w = w_str.trim().parse::<i32>().ok()?.clamp(120, 7680);
    let h = h_str.trim().parse::<i32>().ok()?.clamp(120, 7680);
    Some((w, h))
}

/// Fast pre-Tokio check in `src/main.rs::main()` so `openz browser --worker` and
/// interactive `openz browser [url]` run single-threaded on the true main OS thread
/// with zero Tokio 4-thread / SQLite / ONNX startup overhead.
pub fn maybe_run_browser_cli_from_env_args() -> Option<Result<()>> {
    let args: Vec<String> = std::env::args().collect();
    let is_browser_subcmd = args.get(1).map(String::as_str) == Some("browser");
    match is_browser_subcmd {
        false => return None,
        true => {}
    }
    let passthrough_to_clap = args
        .iter()
        .skip(2)
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "--status"));
    match passthrough_to_clap {
        true => return None,
        false => {}
    }

    let is_probe = args.iter().skip(2).any(|arg| arg == "--probe");
    match is_probe {
        true => {
            let probe = super::webkit_ffi::probe_webkit_runtime();
            let line = serde_json::to_string_pretty(&probe).unwrap_or_else(|_| "{}".to_string());
            println!("{line}");
            return Some(Ok(()));
        }
        false => {}
    }

    Some(parse_and_run_browser_args(&args[2..]))
}

fn parse_and_run_browser_args(raw_args: &[String]) -> Result<()> {
    let cfg = crate::config::loader::load_config().unwrap_or_default();
    let mut worker_mode = false;
    let mut gui_flag: Option<bool> = None;
    let mut session_name = "default".to_string();
    let mut socket_path: Option<PathBuf> = None;
    let mut initial_url: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut width = cfg.browser.default_width.clamp(120, 7680);
    let mut height = cfg.browser.default_height.clamp(120, 7680);
    let mut memory_limit_mb = cfg.browser.memory_limit_mb.clamp(48, 2048);
    let mut idle_timeout_secs = cfg.browser.idle_timeout_secs;
    let mut parent_death_sigterm = false;
    let mut one_shot_screenshot: Option<PathBuf> = None;
    let mut one_shot_pdf: Option<PathBuf> = None;

    let mut idx = 0usize;
    while idx < raw_args.len() {
        let arg = raw_args[idx].as_str();
        match arg {
            "--worker" => worker_mode = true,
            "--gui" | "--visible" | "--headed" => gui_flag = Some(true),
            "--headless" => gui_flag = Some(false),
            "--parent-death-sigterm" => parent_death_sigterm = true,
            "--session" => {
                idx += 1;
                session_name = raw_args
                    .get(idx)
                    .map(|s| sanitize_session_name(s))
                    .unwrap_or_else(|| "default".to_string());
            }
            "--socket" => {
                idx += 1;
                socket_path = raw_args.get(idx).map(PathBuf::from);
            }
            "--profile" => {
                idx += 1;
                profile = raw_args
                    .get(idx)
                    .map(|s| sanitize_session_name(s))
                    .filter(|s| s != "ephemeral" && s != "none");
            }
            "--size" => {
                idx += 1;
                match raw_args.get(idx).and_then(|s| parse_size_spec(s)) {
                    Some((w, h)) => {
                        width = w;
                        height = h;
                    }
                    None => {}
                }
            }
            "--width" => {
                idx += 1;
                width = raw_args
                    .get(idx)
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(width)
                    .clamp(120, 7680);
            }
            "--height" => {
                idx += 1;
                height = raw_args
                    .get(idx)
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(height)
                    .clamp(120, 7680);
            }
            "--memory-mb" => {
                idx += 1;
                memory_limit_mb = raw_args
                    .get(idx)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(memory_limit_mb)
                    .clamp(48, 2048);
            }
            "--idle-timeout" => {
                idx += 1;
                idle_timeout_secs = raw_args
                    .get(idx)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(idle_timeout_secs);
            }
            "--screenshot" => {
                idx += 1;
                one_shot_screenshot = raw_args.get(idx).map(PathBuf::from);
            }
            "--pdf" => {
                idx += 1;
                one_shot_pdf = raw_args.get(idx).map(PathBuf::from);
            }
            "--url" => {
                idx += 1;
                initial_url = raw_args.get(idx).cloned();
            }
            other => {
                let is_flag = other.starts_with('-');
                match (is_flag, initial_url.is_none()) {
                    (false, true) => initial_url = Some(other.to_string()),
                    _ => {}
                }
            }
        }
        idx += 1;
    }

    let gui = gui_flag.unwrap_or(!worker_mode && one_shot_screenshot.is_none() && one_shot_pdf.is_none());
    let resolved_socket = socket_path.unwrap_or_else(|| session_socket_path(&session_name));
    let effective_idle = match gui {
        true => 0,
        false => idle_timeout_secs,
    };

    let opts = BrowserWorkerOptions {
        worker_mode,
        gui,
        socket_path: resolved_socket,
        session_name,
        initial_url: initial_url.map(|u| normalize_cli_url(&u)),
        profile,
        width,
        height,
        memory_limit_mb,
        idle_timeout_secs: effective_idle,
        parent_death_sigterm,
        one_shot_screenshot,
        one_shot_pdf,
    };

    run_browser_worker(opts)
}

pub fn normalize_cli_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let has_scheme = trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
        || trimmed.starts_with("data:")
        || trimmed.starts_with("about:");
    match has_scheme {
        true => trimmed.to_string(),
        false => {
            let as_path = Path::new(trimmed);
            match as_path.exists() {
                true => match as_path.canonicalize() {
                    Ok(abs) => format!("file://{}", abs.to_string_lossy()),
                    Err(_) => format!("https://{trimmed}"),
                },
                false => format!("https://{trimmed}"),
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn run_browser_worker(_opts: BrowserWorkerOptions) -> Result<()> {
    Err(anyhow!(
        "Native WebKit2GTK browser worker is currently supported on Linux hosts"
    ))
}

#[cfg(target_os = "linux")]
pub fn run_browser_worker(opts: BrowserWorkerOptions) -> Result<()> {
    imp::run_linux_worker(opts)
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use crate::tools::browser::webkit_ffi::*;
    use std::collections::{HashMap, VecDeque};
    use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc::{sync_channel, SyncSender};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    struct QueuedIpcCommand {
        cmd: Value,
        reply: SyncSender<Value>,
    }

    struct PendingLoadWaiter {
        settle_ms: u64,
        reply: SyncSender<Value>,
    }

    struct PendingJsCall {
        tolerate_navigation: bool,
        reply: SyncSender<Value>,
    }

    struct SnapshotJob {
        state_ptr: *mut WorkerState,
        path: PathBuf,
        crop_rect: Option<(f64, f64, f64, f64)>,
        reply: SyncSender<Value>,
    }

    struct PdfJob {
        state_ptr: *mut WorkerState,
        path: PathBuf,
        reply: SyncSender<Value>,
    }

    struct WorkerState {
        api: &'static WebKitApi,
        opts: BrowserWorkerOptions,
        queue: Arc<Mutex<VecDeque<QueuedIpcCommand>>>,
        webview: *mut c_void,
        _ucm: *mut c_void,
        offscreen_win: *mut c_void,
        gui_win: *mut c_void,
        gui_vbox: *mut c_void,
        url_entry: *mut c_void,
        header_bar: *mut c_void,
        is_gui: bool,
        suppress_destroy_quit: bool,
        som_active: bool,
        width: i32,
        height: i32,
        zoom: f64,
        next_req_id: u64,
        pending_js: HashMap<u64, PendingJsCall>,
        load_waiters: Vec<PendingLoadWaiter>,
        last_activity: Instant,
        shutting_down: bool,
    }

    struct EphemeralBroadwayGuard {
        child: std::process::Child,
        runtime_dir: PathBuf,
    }

    impl Drop for EphemeralBroadwayGuard {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.runtime_dir);
        }
    }

    fn spawn_ephemeral_broadway_display() -> Result<EphemeralBroadwayGuard> {
        use std::os::unix::process::CommandExt;

        let bin = super::super::webkit_ffi::find_broadwayd_binary().ok_or_else(|| {
            anyhow!(
                "No Wayland/X11 socket reachable and GTK3 broadwayd binary was not found on PATH"
            )
        })?;
        let pid = std::process::id();
        let runtime_dir = std::env::temp_dir().join(format!("openz_bw_{pid}"));
        std::fs::create_dir_all(&runtime_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&runtime_dir, std::fs::Permissions::from_mode(0o700));
        }
        let http_sock = runtime_dir.join("broadway.http");
        let _ = std::fs::remove_file(&http_sock);

        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("DISPLAY");
        std::env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
        std::env::set_var("GDK_BACKEND", "broadway");
        std::env::set_var("BROADWAY_DISPLAY", ":5");

        let mut cmd = std::process::Command::new(bin);
        cmd.arg("-u")
            .arg(&http_sock)
            .arg(":5")
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                let _ = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
        let child = cmd
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn ephemeral broadwayd: {e}"))?;

        let mut attempts = 0u32;
        while attempts < 40 && !http_sock.exists() {
            std::thread::sleep(Duration::from_millis(15));
            attempts += 1;
        }

        Ok(EphemeralBroadwayGuard { child, runtime_dir })
    }

    pub(super) fn run_linux_worker(opts: BrowserWorkerOptions) -> Result<()> {
        match opts.parent_death_sigterm {
            true => unsafe {
                let _ = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            },
            false => {}
        }

        tune_browser_process_env(opts.memory_limit_mb);

        let caller_forced_broadway = std::env::var("GDK_BACKEND")
            .ok()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("broadway"));
        let _broadway_guard = match caller_forced_broadway {
            true => None,
            false => match (detect_display_environment(), opts.gui) {
                (Some(("wayland", sock)), _) => {
                    std::env::set_var("WAYLAND_DISPLAY", sock);
                    None
                }
                (Some(("x11", disp)), _) => {
                    std::env::remove_var("WAYLAND_DISPLAY");
                    std::env::set_var("DISPLAY", disp);
                    None
                }
                (Some(("broadway", _)), false) => Some(spawn_ephemeral_broadway_display()?),
                (_, true) => {
                    return Err(anyhow!(
                        "gtk_init_check failed for GUI mode: no active Wayland or X11 desktop display socket reachable"
                    ));
                }
                (None, false) => None,
                (Some(_), false) => None,
            },
        };

        let api = WebKitApi::get()?;

        let gtk_ok = unsafe { (api.gtk_init_check)(std::ptr::null_mut(), std::ptr::null_mut()) };
        match gtk_ok != 0 {
            true => {}
            false => {
                return Err(anyhow!(
                    "gtk_init_check failed: unable to initialize GTK3 display backend"
                ));
            }
        }

        ensure_positive_screen_dpi(api);
        apply_dark_gtk_css(api);
        configure_memory_pressure(api, opts.memory_limit_mb);

        let web_ctx = create_web_context(api, opts.profile.as_deref())?;
        unsafe {
            (api.webkit_web_context_set_cache_model)(web_ctx, WEBKIT_CACHE_MODEL_DOCUMENT_VIEWER);
        }

        let ucm = unsafe { (api.webkit_user_content_manager_new)() };
        inject_page_controller_user_script(api, ucm)?;
        let channel_c = CString::new("openz")?;
        unsafe {
            (api.webkit_user_content_manager_register_script_message_handler)(
                ucm,
                channel_c.as_ptr(),
            );
        }

        let webview = create_webview_instance(api, web_ctx, ucm)?;
        configure_webview_settings(api, webview, opts.gui);

        let offscreen_win = unsafe { (api.gtk_offscreen_window_new)() };
        unsafe {
            (api.gtk_window_set_default_size)(offscreen_win, opts.width, opts.height);
            (api.gtk_widget_set_size_request)(webview, opts.width, opts.height);
        }

        let queue: Arc<Mutex<VecDeque<QueuedIpcCommand>>> = Arc::new(Mutex::new(VecDeque::new()));

        let boxed_state = Box::new(WorkerState {
            api,
            opts: opts.clone(),
            queue: queue.clone(),
            webview,
            _ucm: ucm,
            offscreen_win,
            gui_win: std::ptr::null_mut(),
            gui_vbox: std::ptr::null_mut(),
            url_entry: std::ptr::null_mut(),
            header_bar: std::ptr::null_mut(),
            is_gui: false,
            suppress_destroy_quit: false,
            som_active: false,
            width: opts.width,
            height: opts.height,
            zoom: 1.0,
            next_req_id: 1,
            pending_js: HashMap::new(),
            load_waiters: Vec::new(),
            last_activity: Instant::now(),
            shutting_down: false,
        });
        let state_ptr = Box::into_raw(boxed_state);

        unsafe {
            connect_signal(
                api,
                ucm,
                "script-message-received::openz",
                std::mem::transmute::<
                    unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void),
                    GCallback,
                >(on_script_message),
                state_ptr as *mut c_void,
            );
            connect_signal(
                api,
                webview,
                "load-changed",
                std::mem::transmute::<
                    unsafe extern "C" fn(*mut c_void, c_int, *mut c_void),
                    GCallback,
                >(on_load_changed),
                state_ptr as *mut c_void,
            );

            match opts.gui {
                true => attach_to_gui_window(&mut *state_ptr),
                false => {
                    (api.gtk_container_add)(offscreen_win, webview);
                    (api.gtk_widget_show_all)(offscreen_win);
                }
            }
        }

        let is_one_shot = !opts.worker_mode
            && !opts.gui
            && (opts.initial_url.is_some()
                || opts.one_shot_screenshot.is_some()
                || opts.one_shot_pdf.is_some());

        let socket_to_cleanup = match is_one_shot {
            true => {
                spawn_one_shot_runner(api, state_ptr, queue.clone());
                None
            }
            false => {
                let listener = bind_worker_socket(&opts.socket_path)?;
                spawn_ipc_accept_thread(api, listener, queue.clone(), state_ptr);
                match (opts.initial_url.as_deref(), opts.gui) {
                    (Some(url), _) => unsafe {
                        load_uri_now(&mut *state_ptr, url);
                    },
                    (None, true) => unsafe {
                        load_html_now(&mut *state_ptr, default_welcome_html(), Some("about:blank"));
                    },
                    (None, false) => {}
                }
                Some(opts.socket_path.clone())
            }
        };

        match opts.idle_timeout_secs > 0 {
            true => unsafe {
                (api.g_timeout_add_full)(
                    G_PRIORITY_DEFAULT_IDLE,
                    10_000,
                    on_idle_watchdog_tick,
                    state_ptr as *mut c_void,
                    None,
                );
            },
            false => {}
        }

        unsafe {
            (api.gtk_main)();
        }

        match socket_to_cleanup {
            Some(sock) => {
                let _ = std::fs::remove_file(sock);
            }
            None => {}
        }

        unsafe {
            let _ = Box::from_raw(state_ptr);
        }
        Ok(())
    }

    fn default_welcome_html() -> &'static str {
        r#"<!doctype html><html><head><meta charset="utf-8"><title>OpenZ Browser</title>
        <style>body{margin:0;height:100vh;display:grid;place-items:center;background:#0b0f19;color:#e2e8f0;font:15px system-ui,sans-serif}
        .card{border:1px solid #1e293b;background:#111827;border-radius:12px;padding:28px 36px;text-align:center;box-shadow:0 12px 32px rgba(0,0,0,.4)}
        h1{margin:0 0 8px;font-size:20px;color:#f8fafc}span{color:#ff4500}p{margin:0;color:#94a3b8;font-size:13px}</style></head>
        <body><div class="card"><h1>Open<span>Z</span> Native Browser</h1><p>WebKit2GTK engine ready · Low-RAM agent &amp; human workspace</p></div></body></html>"#
    }

    fn bind_worker_socket(socket_path: &Path) -> Result<UnixListener> {
        match socket_path.parent() {
            Some(parent) => std::fs::create_dir_all(parent)?,
            None => {}
        }
        match socket_path.exists() {
            true => {
                let _ = std::fs::remove_file(socket_path);
            }
            false => {}
        }
        let listener = UnixListener::bind(socket_path)
            .map_err(|e| anyhow!("Failed to bind browser socket {:?}: {}", socket_path, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(listener)
    }

    fn spawn_ipc_accept_thread(
        api: &'static WebKitApi,
        listener: UnixListener,
        queue: Arc<Mutex<VecDeque<QueuedIpcCommand>>>,
        state_ptr: *mut WorkerState,
    ) {
        let state_addr = state_ptr as usize;
        std::thread::spawn(move || {
            for stream_res in listener.incoming() {
                match stream_res {
                    Ok(stream) => {
                        let q = queue.clone();
                        std::thread::spawn(move || {
                            handle_ipc_client(api, stream, q, state_addr);
                        });
                    }
                    Err(_) => break,
                }
            }
        });
    }

    fn handle_ipc_client(
        api: &'static WebKitApi,
        mut stream: std::os::unix::net::UnixStream,
        queue: Arc<Mutex<VecDeque<QueuedIpcCommand>>>,
        state_addr: usize,
    ) {
        let reader_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut reader = BufReader::new(reader_stream);
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = match reader.read_line(&mut line) {
                Ok(n) => n,
                Err(_) => break,
            };
            match bytes == 0 {
                true => break,
                false => {}
            }
            let trimmed = line.trim();
            match trimmed.is_empty() {
                true => continue,
                false => {}
            }

            let response = match serde_json::from_str::<Value>(trimmed) {
                Ok(cmd_val) => {
                    let (tx, rx) = sync_channel::<Value>(1);
                    match queue.lock() {
                        Ok(mut guard) => guard.push_back(QueuedIpcCommand {
                            cmd: cmd_val,
                            reply: tx,
                        }),
                        Err(_) => break,
                    }
                    unsafe {
                        (api.g_idle_add_full)(
                            G_PRIORITY_DEFAULT_IDLE,
                            drain_command_queue_on_gtk,
                            state_addr as *mut c_void,
                            None,
                        );
                    }
                    rx.recv_timeout(Duration::from_secs(45)).unwrap_or_else(|_| {
                        json!({
                            "ok": false,
                            "error": "Timed out waiting for GTK WebKit worker response"
                        })
                    })
                }
                Err(err) => json!({
                    "ok": false,
                    "error": format!("Invalid IPC JSON request: {err}")
                }),
            };

            let encoded =
                serde_json::to_string(&response).unwrap_or_else(|_| "{\"ok\":false}".to_string());
            match writeln!(stream, "{encoded}").and_then(|_| stream.flush()) {
                Ok(()) => {}
                Err(_) => break,
            }
        }
    }

    fn spawn_one_shot_runner(
        api: &'static WebKitApi,
        state_ptr: *mut WorkerState,
        queue: Arc<Mutex<VecDeque<QueuedIpcCommand>>>,
    ) {
        let state_addr = state_ptr as usize;
        let opts = unsafe { (*state_ptr).opts.clone() };
        std::thread::spawn(move || {
            let send = |cmd: Value| -> Value {
                let (tx, rx) = sync_channel(1);
                match queue.lock() {
                    Ok(mut g) => g.push_back(QueuedIpcCommand { cmd, reply: tx }),
                    Err(_) => return json!({ "ok": false }),
                }
                unsafe {
                    (api.g_idle_add_full)(
                        G_PRIORITY_DEFAULT_IDLE,
                        drain_command_queue_on_gtk,
                        state_addr as *mut c_void,
                        None,
                    );
                }
                rx.recv_timeout(Duration::from_secs(30))
                    .unwrap_or_else(|_| json!({ "ok": false, "error": "one-shot timeout" }))
            };

            let target = opts
                .initial_url
                .clone()
                .unwrap_or_else(|| "https://example.com".to_string());
            let _ = send(json!({
                "op": "navigate",
                "url": target,
                "timeout_ms": 15000,
                "settle_ms": 250
            }));

            match opts.one_shot_screenshot {
                Some(path) => {
                    let res = send(json!({
                        "op": "screenshot",
                        "path": path.to_string_lossy()
                    }));
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&res).unwrap_or_default()
                    );
                }
                None => match opts.one_shot_pdf {
                    Some(path) => {
                        let res = send(json!({
                            "op": "save_pdf",
                            "path": path.to_string_lossy()
                        }));
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&res).unwrap_or_default()
                        );
                    }
                    None => {
                        let res = send(json!({
                            "op": "web_agent",
                            "payload": { "action": "render", "format": "markdown" }
                        }));
                        let md = res
                            .pointer("/data/output")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        println!("{md}");
                    }
                },
            }

            let _ = send(json!({ "op": "stop" }));
        });
    }

    fn ensure_positive_screen_dpi(api: &'static WebKitApi) {
        unsafe {
            let screen = (api.gdk_screen_get_default)();
            let needs_default_dpi = match screen.is_null() {
                true => true,
                false => {
                    let current_dpi = (api.gdk_screen_get_resolution)(screen);
                    let unset = current_dpi <= 0.0;
                    match unset {
                        true => (api.gdk_screen_set_resolution)(screen, 96.0),
                        false => {}
                    }
                    unset
                }
            };
            match needs_default_dpi {
                false => {}
                true => {
                    let gtk_settings = (api.gtk_settings_get_default)();
                    match gtk_settings.is_null() {
                        true => {}
                        false => match CString::new("gtk-xft-dpi") {
                            Ok(prop) => {
                                let dpi_96_pango: c_int = 96 * 1024;
                                (api.g_object_set)(
                                    gtk_settings,
                                    prop.as_ptr(),
                                    dpi_96_pango,
                                    std::ptr::null::<c_char>(),
                                );
                            }
                            Err(_) => {}
                        },
                    }
                }
            }
        }
    }

    fn apply_dark_gtk_css(api: &'static WebKitApi) {
        unsafe {
            let screen = (api.gdk_screen_get_default)();
            match screen.is_null() {
                true => {}
                false => {
                    let provider = (api.gtk_css_provider_new)();
                    let c_css = CString::new(OPENZ_GTK_CSS).unwrap_or_default();
                    let _ = (api.gtk_css_provider_load_from_data)(
                        provider,
                        c_css.as_ptr(),
                        OPENZ_GTK_CSS.len() as isize,
                        std::ptr::null_mut(),
                    );
                    (api.gtk_style_context_add_provider_for_screen)(
                        screen,
                        provider,
                        GTK_STYLE_PROVIDER_PRIORITY_APPLICATION,
                    );
                    (api.g_object_unref)(provider);
                }
            }
        }
    }

    fn configure_memory_pressure(api: &'static WebKitApi, memory_limit_mb: u32) {
        unsafe {
            let settings = (api.webkit_memory_pressure_settings_new)();
            match settings.is_null() {
                true => {}
                false => {
                    (api.webkit_memory_pressure_settings_set_memory_limit)(
                        settings,
                        memory_limit_mb.clamp(48, 2048),
                    );
                    (api.webkit_memory_pressure_settings_set_conservative_threshold)(
                        settings, 0.35,
                    );
                    (api.webkit_memory_pressure_settings_set_strict_threshold)(settings, 0.60);
                    (api.webkit_memory_pressure_settings_set_kill_threshold)(settings, 0.90);
                    (api.webkit_website_data_manager_set_memory_pressure_settings)(settings);
                    (api.webkit_memory_pressure_settings_free)(settings);
                }
            }
        }
    }

    fn create_web_context(api: &'static WebKitApi, profile: Option<&str>) -> Result<*mut c_void> {
        match profile {
            None => {
                let ctx = unsafe { (api.webkit_web_context_new_ephemeral)() };
                match ctx.is_null() {
                    true => Err(anyhow!("webkit_web_context_new_ephemeral returned null")),
                    false => Ok(ctx),
                }
            }
            Some(prof_name) => {
                let root = crate::config::config_dir()
                    .join("browser_profiles")
                    .join(sanitize_session_name(prof_name));
                let data_dir = root.join("data");
                let cache_dir = root.join("cache");
                std::fs::create_dir_all(&data_dir)?;
                std::fs::create_dir_all(&cache_dir)?;

                let prop_data = CString::new("base-data-directory")?;
                let prop_cache = CString::new("base-cache-directory")?;
                let val_data = CString::new(data_dir.to_string_lossy().as_bytes())?;
                let val_cache = CString::new(cache_dir.to_string_lossy().as_bytes())?;

                let mgr = unsafe {
                    (api.webkit_website_data_manager_new)(
                        prop_data.as_ptr(),
                        val_data.as_ptr(),
                        prop_cache.as_ptr(),
                        val_cache.as_ptr(),
                        std::ptr::null::<c_char>(),
                    )
                };
                match mgr.is_null() {
                    true => create_web_context(api, None),
                    false => {
                        let ctx =
                            unsafe { (api.webkit_web_context_new_with_website_data_manager)(mgr) };
                        unsafe {
                            (api.g_object_unref)(mgr);
                        }
                        match ctx.is_null() {
                            true => create_web_context(api, None),
                            false => Ok(ctx),
                        }
                    }
                }
            }
        }
    }

    fn inject_page_controller_user_script(
        api: &'static WebKitApi,
        ucm: *mut c_void,
    ) -> Result<()> {
        let c_source = CString::new(PAGE_CONTROLLER_JS)?;
        unsafe {
            let script = (api.webkit_user_script_new)(
                c_source.as_ptr(),
                WEBKIT_INJECT_ALL_FRAMES,
                WEBKIT_INJECT_AT_DOCUMENT_START,
                std::ptr::null(),
                std::ptr::null(),
            );
            match script.is_null() {
                true => Err(anyhow!("webkit_user_script_new returned null")),
                false => {
                    (api.webkit_user_content_manager_add_script)(ucm, script);
                    (api.webkit_user_script_unref)(script);
                    Ok(())
                }
            }
        }
    }

    fn create_webview_instance(
        api: &'static WebKitApi,
        web_ctx: *mut c_void,
        ucm: *mut c_void,
    ) -> Result<*mut c_void> {
        let p_ctx = CString::new("web-context")?;
        let p_ucm = CString::new("user-content-manager")?;
        unsafe {
            let gtype = (api.webkit_web_view_get_type)();
            let webview = (api.g_object_new)(
                gtype,
                p_ctx.as_ptr(),
                web_ctx,
                p_ucm.as_ptr(),
                ucm,
                std::ptr::null::<c_char>(),
            );
            match webview.is_null() {
                true => Err(anyhow!("Failed to instantiate WebKitWebView")),
                false => {
                    // Hold a sunk strong reference so reparenting headless <-> GUI never drops webview
                    (api.g_object_ref_sink)(webview);
                    Ok(webview)
                }
            }
        }
    }

    fn configure_webview_settings(api: &'static WebKitApi, webview: *mut c_void, is_gui: bool) {
        unsafe {
            let settings = (api.webkit_settings_new)();
            match settings.is_null() {
                true => {}
                false => {
                    (api.webkit_settings_set_enable_javascript)(settings, 1);
                    (api.webkit_settings_set_enable_webgl)(settings, 0);
                    (api.webkit_settings_set_enable_media_stream)(settings, 0);
                    (api.webkit_settings_set_enable_developer_extras)(
                        settings,
                        match is_gui {
                            true => 1,
                            false => 0,
                        },
                    );
                    (api.webkit_settings_set_hardware_acceleration_policy)(
                        settings,
                        WEBKIT_HARDWARE_ACCELERATION_POLICY_NEVER,
                    );
                    let ua_str = crate::config::loader::load_config()
                        .map(|c| c.web.user_agent)
                        .ok()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| {
                            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) OpenZBrowser/1.0 Safari/605.1.15".to_string()
                        });
                    match CString::new(ua_str) {
                        Ok(c_ua) => (api.webkit_settings_set_user_agent)(settings, c_ua.as_ptr()),
                        Err(_) => {}
                    }
                    (api.webkit_web_view_set_settings)(webview, settings);
                    (api.g_object_unref)(settings);
                }
            }
        }
    }

    unsafe fn connect_signal(
        api: &'static WebKitApi,
        instance: *mut c_void,
        signal_name: &str,
        callback: GCallback,
        user_data: *mut c_void,
    ) {
        match CString::new(signal_name) {
            Ok(c_sig) => {
                let _ = (api.g_signal_connect_data)(
                    instance,
                    c_sig.as_ptr(),
                    callback,
                    user_data,
                    None,
                    0,
                );
            }
            Err(_) => {}
        }
    }

    unsafe fn attach_to_gui_window(st: &mut WorkerState) {
        let api = st.api;
        match st.is_gui && !st.gui_win.is_null() {
            true => {
                (api.gtk_widget_show_all)(st.gui_win);
                return;
            }
            false => {}
        }

        // Detach from offscreen window when previously attached there
        match !st.is_gui && !st.gui_win.is_null() {
            true => {
                (api.gtk_container_remove)(st.offscreen_win, st.webview);
            }
            false => {}
        }

        match st.gui_win.is_null() {
            true => {
                // If webview was already packed in offscreen_win, detach first
                let alloc_w = (api.gtk_widget_get_allocated_width)(st.webview);
                match alloc_w > 1 {
                    true => (api.gtk_container_remove)(st.offscreen_win, st.webview),
                    false => {}
                }
                build_gui_shell(st);
            }
            false => {
                let vbox_children_box = st.gui_win;
                let _ = vbox_children_box;
            }
        }

        st.is_gui = true;
        (api.gtk_widget_show_all)(st.gui_win);
        eval_fire_and_forget(
            st,
            "window.__openzBrowser && window.__openzBrowser.dispatch({action:'takeover_mask',enabled:true,status_text:'OpenZ Browser Ready'});",
        );
    }

    unsafe fn build_gui_shell(st: &mut WorkerState) {
        let api = st.api;
        let state_ptr = st as *mut WorkerState as *mut c_void;

        let win = (api.gtk_window_new)(GTK_WINDOW_TOPLEVEL);
        let title_c = CString::new(format!("OpenZ Browser — {}", st.opts.session_name)).unwrap_or_default();
        (api.gtk_window_set_title)(win, title_c.as_ptr());
        (api.gtk_window_set_default_size)(win, st.width, st.height + 48);

        let hb = (api.gtk_header_bar_new)();
        (api.gtk_header_bar_set_show_close_button)(hb, 1);
        let hb_title = CString::new("OpenZ Browser").unwrap_or_default();
        let hb_sub = CString::new(format!("session: {}", st.opts.session_name)).unwrap_or_default();
        (api.gtk_header_bar_set_title)(hb, hb_title.as_ptr());
        (api.gtk_header_bar_set_subtitle)(hb, hb_sub.as_ptr());

        let nav_box = (api.gtk_box_new)(GTK_ORIENTATION_HORIZONTAL, 4);
        let btn_back = make_button(api, "◀", on_btn_back, state_ptr);
        let btn_fwd = make_button(api, "▶", on_btn_forward, state_ptr);
        let btn_reload = make_button(api, "↻", on_btn_reload, state_ptr);
        (api.gtk_box_pack_start)(nav_box, btn_back, 0, 0, 0);
        (api.gtk_box_pack_start)(nav_box, btn_fwd, 0, 0, 0);
        (api.gtk_box_pack_start)(nav_box, btn_reload, 0, 0, 0);
        (api.gtk_header_bar_pack_start)(hb, nav_box);

        let url_entry = (api.gtk_entry_new)();
        (api.gtk_widget_set_size_request)(url_entry, 420, -1);
        let ph = CString::new("Enter URL (https://...) and press Enter").unwrap_or_default();
        (api.gtk_entry_set_placeholder_text)(url_entry, ph.as_ptr());
        connect_signal(
            api,
            url_entry,
            "activate",
            std::mem::transmute::<unsafe extern "C" fn(*mut c_void, *mut c_void), GCallback>(
                on_url_entry_activate,
            ),
            state_ptr,
        );
        (api.gtk_header_bar_pack_start)(hb, url_entry);

        let tools_box = (api.gtk_box_new)(GTK_ORIENTATION_HORIZONTAL, 4);
        let btn_marks = make_button(api, "[0] Marks", on_btn_toggle_marks, state_ptr);
        let btn_desktop = make_button(api, "Desktop", on_btn_size_desktop, state_ptr);
        let btn_mobile = make_button(api, "Mobile", on_btn_size_mobile, state_ptr);
        (api.gtk_box_pack_start)(tools_box, btn_marks, 0, 0, 0);
        (api.gtk_box_pack_start)(tools_box, btn_desktop, 0, 0, 0);
        (api.gtk_box_pack_start)(tools_box, btn_mobile, 0, 0, 0);
        (api.gtk_header_bar_pack_end)(hb, tools_box);

        (api.gtk_window_set_titlebar)(win, hb);

        let root_vbox = (api.gtk_box_new)(GTK_ORIENTATION_VERTICAL, 0);
        (api.gtk_box_pack_start)(root_vbox, st.webview, 1, 1, 0);
        (api.gtk_container_add)(win, root_vbox);

        connect_signal(
            api,
            win,
            "destroy",
            std::mem::transmute::<unsafe extern "C" fn(*mut c_void, *mut c_void), GCallback>(
                on_gui_window_destroy,
            ),
            state_ptr,
        );

        st.gui_win = win;
        st.gui_vbox = root_vbox;
        st.url_entry = url_entry;
        st.header_bar = hb;
    }

    unsafe fn make_button(
        api: &'static WebKitApi,
        label: &str,
        handler: unsafe extern "C" fn(*mut c_void, *mut c_void),
        user_data: *mut c_void,
    ) -> *mut c_void {
        let c_lbl = CString::new(label).unwrap_or_default();
        let btn = (api.gtk_button_new_with_label)(c_lbl.as_ptr());
        connect_signal(
            api,
            btn,
            "clicked",
            std::mem::transmute::<unsafe extern "C" fn(*mut c_void, *mut c_void), GCallback>(
                handler,
            ),
            user_data,
        );
        btn
    }

    unsafe extern "C" fn on_btn_back(_btn: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        match (st.api.webkit_web_view_can_go_back)(st.webview) != 0 {
            true => (st.api.webkit_web_view_go_back)(st.webview),
            false => {}
        }
    }

    unsafe extern "C" fn on_btn_forward(_btn: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        match (st.api.webkit_web_view_can_go_forward)(st.webview) != 0 {
            true => (st.api.webkit_web_view_go_forward)(st.webview),
            false => {}
        }
    }

    unsafe extern "C" fn on_btn_reload(_btn: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        (st.api.webkit_web_view_reload)(st.webview);
    }

    unsafe extern "C" fn on_url_entry_activate(entry: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        let raw_ptr = (st.api.gtk_entry_get_text)(entry);
        match raw_ptr.is_null() {
            true => {}
            false => {
                let text = CStr::from_ptr(raw_ptr).to_string_lossy().to_string();
                let trimmed = text.trim();
                match trimmed.is_empty() {
                    true => {}
                    false => {
                        let url = normalize_cli_url(trimmed);
                        load_uri_now(st, &url);
                    }
                }
            }
        }
    }

    unsafe extern "C" fn on_btn_toggle_marks(_btn: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        st.som_active = !st.som_active;
        let flag = match st.som_active {
            true => "true",
            false => "false",
        };
        eval_fire_and_forget(
            st,
            &format!(
                "window.__openzBrowser && window.__openzBrowser.dispatch({{action:'inspect_overlay',enabled:{flag}}});"
            ),
        );
    }

    unsafe extern "C" fn on_btn_size_desktop(_btn: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        apply_viewport_size(st, 1280, 800, 1.0);
    }

    unsafe extern "C" fn on_btn_size_mobile(_btn: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        apply_viewport_size(st, 412, 915, 1.0);
    }

    unsafe extern "C" fn on_gui_window_destroy(_win: *mut c_void, user_data: *mut c_void) {
        let st = &mut *(user_data as *mut WorkerState);
        st.gui_win = std::ptr::null_mut();
        st.gui_vbox = std::ptr::null_mut();
        st.url_entry = std::ptr::null_mut();
        st.header_bar = std::ptr::null_mut();
        match st.suppress_destroy_quit {
            true => {}
            false => {
                st.shutting_down = true;
                (st.api.gtk_main_quit)();
            }
        }
    }

    unsafe extern "C" fn on_idle_watchdog_tick(user_data: *mut c_void) -> c_int {
        let st = &mut *(user_data as *mut WorkerState);
        match st.shutting_down {
            true => return 0,
            false => {}
        }
        match st.is_gui || st.opts.idle_timeout_secs == 0 {
            true => return 1,
            false => {}
        }
        let idle_expired = st.pending_js.is_empty()
            && st.load_waiters.is_empty()
            && st.last_activity.elapsed() >= Duration::from_secs(st.opts.idle_timeout_secs);
        match idle_expired {
            true => {
                st.shutting_down = true;
                (st.api.gtk_main_quit)();
                0
            }
            false => {
                let _ = libc::malloc_trim(0);
                1
            }
        }
    }

    unsafe fn current_uri_and_title(st: &WorkerState) -> (String, String, bool) {
        let uri_ptr = (st.api.webkit_web_view_get_uri)(st.webview);
        let uri = match uri_ptr.is_null() {
            true => String::new(),
            false => CStr::from_ptr(uri_ptr).to_string_lossy().to_string(),
        };
        let title_ptr = (st.api.webkit_web_view_get_title)(st.webview);
        let title = match title_ptr.is_null() {
            true => String::new(),
            false => CStr::from_ptr(title_ptr).to_string_lossy().to_string(),
        };
        let loading = (st.api.webkit_web_view_is_loading)(st.webview) != 0;
        (uri, title, loading)
    }

    unsafe fn sync_gui_chrome(st: &mut WorkerState) {
        match st.is_gui {
            false => return,
            true => {}
        }
        let (uri, title, _) = current_uri_and_title(st);
        match (!st.url_entry.is_null(), !uri.is_empty()) {
            (true, true) => match CString::new(uri.as_bytes()) {
                Ok(c_uri) => (st.api.gtk_entry_set_text)(st.url_entry, c_uri.as_ptr()),
                Err(_) => {}
            },
            _ => {}
        }
        match (!st.header_bar.is_null(), !title.is_empty()) {
            (true, true) => match CString::new(title.as_bytes()) {
                Ok(c_title) => (st.api.gtk_header_bar_set_title)(st.header_bar, c_title.as_ptr()),
                Err(_) => {}
            },
            _ => {}
        }
    }

    unsafe extern "C" fn on_load_changed(
        _webview: *mut c_void,
        load_event: c_int,
        user_data: *mut c_void,
    ) {
        let st = &mut *(user_data as *mut WorkerState);
        sync_gui_chrome(st);

        match load_event == WEBKIT_LOAD_FINISHED {
            false => return,
            true => {}
        }

        match st.is_gui {
            true => eval_fire_and_forget(
                st,
                "window.__openzBrowser && window.__openzBrowser.dispatch({action:'takeover_mask',enabled:true,status_text:'OpenZ Browser Ready'});",
            ),
            false => {}
        }

        // Complete any navigation-tolerant JS calls whose old JS context navigated away
        let (uri, title, loading) = current_uri_and_title(st);
        let nav_ids: Vec<u64> = st
            .pending_js
            .iter()
            .filter_map(|(&id, call)| call.tolerate_navigation.then_some(id))
            .collect();
        for id in nav_ids {
            match st.pending_js.remove(&id) {
                Some(call) => {
                    let _ = call.reply.send(json!({
                        "ok": true,
                        "data": {
                            "navigated": true,
                            "url": uri,
                            "title": title,
                            "ui_diff": format!("Navigated to {uri}")
                        }
                    }));
                }
                None => {}
            }
        }

        let waiters: Vec<PendingLoadWaiter> = st.load_waiters.drain(..).collect();
        let state_ptr = st as *mut WorkerState;
        for waiter in waiters {
            match waiter.settle_ms == 0 {
                true => {
                    let _ = waiter.reply.send(json!({
                        "ok": true,
                        "url": uri,
                        "title": title,
                        "loading": loading
                    }));
                }
                false => {
                    let settle_box = Box::new((state_ptr, waiter.reply));
                    (st.api.g_timeout_add_full)(
                        G_PRIORITY_DEFAULT_IDLE,
                        waiter.settle_ms.min(10_000) as c_uint,
                        on_load_settle_complete,
                        Box::into_raw(settle_box) as *mut c_void,
                        None,
                    );
                }
            }
        }
    }

    unsafe extern "C" fn on_load_settle_complete(user_data: *mut c_void) -> c_int {
        let pair = Box::from_raw(user_data as *mut (*mut WorkerState, SyncSender<Value>));
        let st = &*pair.0;
        let (uri, title, loading) = current_uri_and_title(st);
        let _ = pair.1.send(json!({
            "ok": true,
            "url": uri,
            "title": title,
            "loading": loading
        }));
        0
    }

    unsafe extern "C" fn on_script_message(
        _ucm: *mut c_void,
        js_result: *mut c_void,
        user_data: *mut c_void,
    ) {
        let st = &mut *(user_data as *mut WorkerState);
        match js_result.is_null() {
            true => return,
            false => {}
        }
        let jsc_val = (st.api.webkit_javascript_result_get_js_value)(js_result);
        match jsc_val.is_null() {
            true => return,
            false => {}
        }
        let str_ptr = (st.api.jsc_value_to_string)(jsc_val);
        match str_ptr.is_null() {
            true => return,
            false => {}
        }
        let json_str = CStr::from_ptr(str_ptr).to_string_lossy().to_string();
        (st.api.g_free)(str_ptr as *mut c_void);

        let parsed = match serde_json::from_str::<Value>(&json_str) {
            Ok(v) => v,
            Err(_) => return,
        };
        let req_id = match parsed.get("req_id").and_then(|v| v.as_u64()) {
            Some(id) => id,
            None => return,
        };
        match st.pending_js.remove(&req_id) {
            Some(call) => {
                let ok = parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                let reply_payload = match ok {
                    true => json!({
                        "ok": true,
                        "data": parsed.get("data").cloned().unwrap_or(Value::Null)
                    }),
                    false => json!({
                        "ok": false,
                        "error": parsed.get("error").and_then(|v| v.as_str()).unwrap_or("JS execution failed")
                    }),
                };
                let _ = call.reply.send(reply_payload);
            }
            None => {}
        }
    }

    unsafe fn eval_fire_and_forget(st: &WorkerState, script: &str) {
        match CString::new(script) {
            Ok(c_js) => {
                (st.api.webkit_web_view_run_javascript)(
                    st.webview,
                    c_js.as_ptr(),
                    std::ptr::null_mut(),
                    None,
                    std::ptr::null_mut(),
                );
            }
            Err(_) => {}
        }
    }

    unsafe fn load_uri_now(st: &mut WorkerState, url: &str) {
        match CString::new(url) {
            Ok(c_url) => (st.api.webkit_web_view_load_uri)(st.webview, c_url.as_ptr()),
            Err(_) => {}
        }
    }

    unsafe fn load_html_now(st: &mut WorkerState, html: &str, base_uri: Option<&str>) {
        let clean_html = html.replace('\0', "");
        let c_html = match CString::new(clean_html) {
            Ok(c) => c,
            Err(_) => return,
        };
        let c_base = base_uri.and_then(|b| CString::new(b).ok());
        let base_ptr = c_base
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());
        (st.api.webkit_web_view_load_html)(st.webview, c_html.as_ptr(), base_ptr);
    }

    unsafe fn apply_viewport_size(st: &mut WorkerState, width: i32, height: i32, scale: f64) {
        let clamped_w = width.clamp(120, 7680);
        let clamped_h = height.clamp(120, 7680);
        let clamped_scale = scale.clamp(0.25, 4.0);
        let phys_w = ((clamped_w as f64) * clamped_scale).round() as i32;
        let phys_h = ((clamped_h as f64) * clamped_scale).round() as i32;

        st.width = clamped_w;
        st.height = clamped_h;
        st.zoom = clamped_scale;

        (st.api.webkit_web_view_set_zoom_level)(st.webview, clamped_scale);
        (st.api.gtk_widget_set_size_request)(st.webview, phys_w, phys_h);
        match st.is_gui && !st.gui_win.is_null() {
            true => (st.api.gtk_window_resize)(st.gui_win, phys_w, phys_h + 48),
            false => (st.api.gtk_window_resize)(st.offscreen_win, phys_w, phys_h),
        }
    }

    unsafe extern "C" fn drain_command_queue_on_gtk(user_data: *mut c_void) -> c_int {
        let st = &mut *(user_data as *mut WorkerState);
        loop {
            let next_item = match st.queue.lock() {
                Ok(mut guard) => guard.pop_front(),
                Err(_) => None,
            };
            match next_item {
                Some(item) => dispatch_ipc_command(st, item.cmd, item.reply),
                None => break,
            }
        }
        0
    }

    unsafe fn dispatch_ipc_command(st: &mut WorkerState, cmd: Value, reply: SyncSender<Value>) {
        st.last_activity = Instant::now();
        let op = cmd.get("op").and_then(|v| v.as_str()).unwrap_or("status");

        match op {
            "status" | "ping" => {
                let (uri, title, loading) = current_uri_and_title(st);
                let progress = (st.api.webkit_web_view_get_estimated_load_progress)(st.webview);
                let _ = reply.send(json!({
                    "ok": true,
                    "session": st.opts.session_name,
                    "pid": std::process::id(),
                    "mode": match st.is_gui { true => "gui", false => "headless" },
                    "webkit_soname": st.api.webkit_soname,
                    "profile": st.opts.profile.as_deref().unwrap_or("ephemeral"),
                    "url": uri,
                    "title": title,
                    "loading": loading,
                    "load_progress": progress,
                    "viewport": {
                        "width": st.width,
                        "height": st.height,
                        "scale": st.zoom,
                    },
                    "memory_limit_mb": st.opts.memory_limit_mb,
                    "rss_kb": read_process_rss_kb(),
                }));
            }
            "navigate" => {
                let timeout_ms = cmd
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(12_000)
                    .clamp(200, 40_000);
                let settle_ms = cmd
                    .get("settle_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(150)
                    .min(10_000);

                match cmd.get("width").and_then(|v| v.as_i64()).zip(
                    cmd.get("height").and_then(|v| v.as_i64()),
                ) {
                    Some((w, h)) => {
                        let scale = cmd.get("scale").and_then(|v| v.as_f64()).unwrap_or(st.zoom);
                        apply_viewport_size(st, w as i32, h as i32, scale);
                    }
                    None => {}
                }

                let nav_kind = cmd.get("nav").and_then(|v| v.as_str()).unwrap_or("");
                match nav_kind {
                    "back" => {
                        (st.api.webkit_web_view_go_back)(st.webview);
                    }
                    "forward" => {
                        (st.api.webkit_web_view_go_forward)(st.webview);
                    }
                    "reload" => {
                        (st.api.webkit_web_view_reload)(st.webview);
                    }
                    _ => match cmd.get("html").and_then(|v| v.as_str()) {
                        Some(html) => {
                            let base_uri = cmd.get("base_uri").and_then(|v| v.as_str());
                            load_html_now(st, html, base_uri);
                        }
                        None => match cmd.get("url").and_then(|v| v.as_str()) {
                            Some(url) => load_uri_now(st, url),
                            None => {
                                let _ = reply.send(json!({
                                    "ok": false,
                                    "error": "navigate requires url, html, or nav=back|forward|reload"
                                }));
                                return;
                            }
                        },
                    },
                }

                st.load_waiters.push(PendingLoadWaiter {
                    settle_ms,
                    reply: reply.clone(),
                });
                let state_ptr = st as *mut WorkerState;
                let timeout_box = Box::new((state_ptr, reply));
                (st.api.g_timeout_add_full)(
                    G_PRIORITY_DEFAULT_IDLE,
                    timeout_ms as c_uint,
                    on_navigate_deadline,
                    Box::into_raw(timeout_box) as *mut c_void,
                    None,
                );
            }
            "resize" => {
                let w = cmd
                    .get("width")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(st.width as i64) as i32;
                let h = cmd
                    .get("height")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(st.height as i64) as i32;
                let scale = cmd.get("scale").and_then(|v| v.as_f64()).unwrap_or(st.zoom);
                apply_viewport_size(st, w, h, scale);
                let _ = reply.send(json!({
                    "ok": true,
                    "viewport": { "width": st.width, "height": st.height, "scale": st.zoom }
                }));
            }
            "mode" => {
                let target_mode = cmd.get("mode").and_then(|v| v.as_str()).unwrap_or("headless");
                let want_gui = matches!(target_mode, "gui" | "visible" | "headed");
                match (want_gui, st.is_gui) {
                    (true, false) => {
                        (st.api.gtk_container_remove)(st.offscreen_win, st.webview);
                        build_gui_shell(st);
                        st.is_gui = true;
                        (st.api.gtk_widget_show_all)(st.gui_win);
                        sync_gui_chrome(st);
                    }
                    (false, true) => {
                        match !st.gui_vbox.is_null() {
                            true => (st.api.gtk_container_remove)(st.gui_vbox, st.webview),
                            false => {}
                        }
                        match !st.gui_win.is_null() {
                            true => {
                                st.suppress_destroy_quit = true;
                                (st.api.gtk_widget_destroy)(st.gui_win);
                                st.suppress_destroy_quit = false;
                                st.gui_win = std::ptr::null_mut();
                                st.gui_vbox = std::ptr::null_mut();
                                st.url_entry = std::ptr::null_mut();
                                st.header_bar = std::ptr::null_mut();
                            }
                            false => {}
                        }
                        (st.api.gtk_container_add)(st.offscreen_win, st.webview);
                        (st.api.gtk_widget_show_all)(st.offscreen_win);
                        st.is_gui = false;
                    }
                    _ => {}
                }
                let _ = reply.send(json!({
                    "ok": true,
                    "mode": match st.is_gui { true => "gui", false => "headless" }
                }));
            }
            "web_agent" => {
                let payload = cmd.get("payload").cloned().unwrap_or_else(|| json!({}));
                let action_str = payload
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ui_tree");
                let tolerate_navigation =
                    matches!(action_str, "click" | "type" | "fill" | "fill_form");
                let timeout_ms = payload
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(15_000)
                    .clamp(300, 40_000);

                let req_id = st.next_req_id;
                st.next_req_id += 1;
                st.pending_js.insert(
                    req_id,
                    PendingJsCall {
                        tolerate_navigation,
                        reply,
                    },
                );

                let payload_json =
                    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
                let bootstrap_json =
                    serde_json::to_string(PAGE_CONTROLLER_JS).unwrap_or_else(|_| "\"\"".to_string());
                let wrapper_js = format!(
                    r#"(async () => {{
                        const __post = (msg) => {{
                            try {{ window.webkit.messageHandlers.openz.postMessage(JSON.stringify(msg)); }} catch (_) {{}}
                        }};
                        try {{
                            if (!window.__openzBrowser) {{
                                (0, eval)({bootstrap_json});
                            }}
                            const res = await window.__openzBrowser.dispatch({payload_json});
                            __post({{ req_id: {req_id}, ok: true, data: res }});
                        }} catch (err) {{
                            __post({{ req_id: {req_id}, ok: false, error: String(err && err.message ? err.message : err) }});
                        }}
                    }})();"#
                );
                eval_fire_and_forget(st, &wrapper_js);

                let state_ptr = st as *mut WorkerState;
                let timer_box = Box::new((state_ptr, req_id));
                (st.api.g_timeout_add_full)(
                    G_PRIORITY_DEFAULT_IDLE,
                    timeout_ms as c_uint,
                    on_js_call_timeout,
                    Box::into_raw(timer_box) as *mut c_void,
                    None,
                );
            }
            "screenshot" => {
                let path_str = match cmd.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => {
                        let _ = reply.send(json!({ "ok": false, "error": "screenshot requires path" }));
                        return;
                    }
                };
                let full_page = cmd.get("full_page").and_then(|v| v.as_bool()).unwrap_or(false);
                let transparent = cmd
                    .get("transparent")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let crop_rect = parse_crop_rect(&cmd, st.zoom);

                match transparent {
                    true => {
                        let bg = GdkRGBA {
                            red: 0.0,
                            green: 0.0,
                            blue: 0.0,
                            alpha: 0.0,
                        };
                        (st.api.webkit_web_view_set_background_color)(st.webview, &bg);
                    }
                    false => {}
                }

                let region = match full_page || crop_rect.is_some() {
                    true => WEBKIT_SNAPSHOT_REGION_FULL_DOCUMENT,
                    false => WEBKIT_SNAPSHOT_REGION_VISIBLE,
                };
                let options = match transparent {
                    true => WEBKIT_SNAPSHOT_OPTIONS_TRANSPARENT_BACKGROUND,
                    false => WEBKIT_SNAPSHOT_OPTIONS_NONE,
                };

                let job = Box::new(SnapshotJob {
                    state_ptr: st as *mut WorkerState,
                    path: PathBuf::from(path_str),
                    crop_rect,
                    reply,
                });
                (st.api.webkit_web_view_get_snapshot)(
                    st.webview,
                    region,
                    options,
                    std::ptr::null_mut(),
                    Some(on_snapshot_ready),
                    Box::into_raw(job) as *mut c_void,
                );
            }
            "save_pdf" => {
                let path_str = match cmd.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => {
                        let _ = reply.send(json!({ "ok": false, "error": "save_pdf requires path" }));
                        return;
                    }
                };
                let job = Box::new(PdfJob {
                    state_ptr: st as *mut WorkerState,
                    path: PathBuf::from(path_str),
                    reply,
                });
                (st.api.webkit_web_view_get_snapshot)(
                    st.webview,
                    WEBKIT_SNAPSHOT_REGION_FULL_DOCUMENT,
                    WEBKIT_SNAPSHOT_OPTIONS_NONE,
                    std::ptr::null_mut(),
                    Some(on_pdf_snapshot_ready),
                    Box::into_raw(job) as *mut c_void,
                );
            }
            "stop" | "close" => {
                st.shutting_down = true;
                let _ = reply.send(json!({
                    "ok": true,
                    "stopped": true,
                    "session": st.opts.session_name
                }));
                (st.api.gtk_main_quit)();
            }
            other => {
                let _ = reply.send(json!({
                    "ok": false,
                    "error": format!("Unsupported worker op: {other}")
                }));
            }
        }
    }

    fn parse_crop_rect(cmd: &Value, zoom: f64) -> Option<(f64, f64, f64, f64)> {
        let crop = cmd.get("crop")?;
        let x = crop.get("x")?.as_f64()? * zoom;
        let y = crop.get("y")?.as_f64()? * zoom;
        let w = (crop.get("w")?.as_f64()? * zoom).max(1.0);
        let h = (crop.get("h")?.as_f64()? * zoom).max(1.0);
        Some((x, y, w, h))
    }

    unsafe extern "C" fn on_navigate_deadline(user_data: *mut c_void) -> c_int {
        let pair = Box::from_raw(user_data as *mut (*mut WorkerState, SyncSender<Value>));
        let st = &mut *pair.0;
        let (uri, title, loading) = current_uri_and_title(st);
        // Drain matching waiter if still waiting so load timeout resolves cleanly with partial page
        st.load_waiters.clear();
        let _ = pair.1.send(json!({
            "ok": true,
            "url": uri,
            "title": title,
            "loading": loading,
            "timed_out": true
        }));
        0
    }

    unsafe extern "C" fn on_js_call_timeout(user_data: *mut c_void) -> c_int {
        let pair = Box::from_raw(user_data as *mut (*mut WorkerState, u64));
        let st = &mut *pair.0;
        match st.pending_js.remove(&pair.1) {
            Some(call) => {
                let _ = call.reply.send(json!({
                    "ok": false,
                    "error": "Timed out waiting for page JavaScript execution"
                }));
            }
            None => {}
        }
        0
    }

    unsafe fn fallback_widget_surface(st: &WorkerState) -> *mut c_void {
        let w = ((st.width as f64) * st.zoom).round().max(1.0) as c_int;
        let h = ((st.height as f64) * st.zoom).round().max(1.0) as c_int;
        let surf = (st.api.cairo_image_surface_create)(CAIRO_FORMAT_ARGB32, w, h);
        match surf.is_null() {
            true => std::ptr::null_mut(),
            false => {
                let cr = (st.api.cairo_create)(surf);
                (st.api.gtk_widget_draw)(st.webview, cr);
                (st.api.cairo_destroy)(cr);
                (st.api.cairo_surface_flush)(surf);
                surf
            }
        }
    }

    unsafe extern "C" fn on_snapshot_ready(
        _source_object: *mut c_void,
        res: *mut c_void,
        user_data: *mut c_void,
    ) {
        let job = Box::from_raw(user_data as *mut SnapshotJob);
        let st = &mut *job.state_ptr;
        let api = st.api;

        let mut err_ptr: *mut GError = std::ptr::null_mut();
        let raw_surface =
            (api.webkit_web_view_get_snapshot_finish)(st.webview, res, &mut err_ptr);
        match err_ptr.is_null() {
            true => {}
            false => (api.g_error_free)(err_ptr),
        }

        let surface = match raw_surface.is_null() {
            false => raw_surface,
            true => fallback_widget_surface(st),
        };
        match surface.is_null() {
            true => {
                let _ = job.reply.send(json!({
                    "ok": false,
                    "error": "Failed to capture Cairo surface snapshot from WebKitWebView"
                }));
                return;
            }
            false => {}
        }

        let target_surface = match job.crop_rect {
            None => surface,
            Some((cx, cy, cw, ch)) => {
                let out_w = cw.round().max(1.0) as c_int;
                let out_h = ch.round().max(1.0) as c_int;
                let cropped = (api.cairo_image_surface_create)(CAIRO_FORMAT_ARGB32, out_w, out_h);
                let cr = (api.cairo_create)(cropped);
                (api.cairo_set_source_surface)(cr, surface, -cx, -cy);
                (api.cairo_paint)(cr);
                (api.cairo_destroy)(cr);
                (api.cairo_surface_flush)(cropped);
                (api.cairo_surface_destroy)(surface);
                cropped
            }
        };

        match job.path.parent() {
            Some(parent) => {
                let _ = std::fs::create_dir_all(parent);
            }
            None => {}
        }

        let out_w = (api.cairo_image_surface_get_width)(target_surface);
        let out_h = (api.cairo_image_surface_get_height)(target_surface);
        let write_status = match CString::new(job.path.to_string_lossy().as_bytes()) {
            Ok(c_path) => (api.cairo_surface_write_to_png)(target_surface, c_path.as_ptr()),
            Err(_) => -1,
        };
        (api.cairo_surface_destroy)(target_surface);
        let _ = libc::malloc_trim(0);

        match write_status == CAIRO_STATUS_SUCCESS {
            true => {
                let _ = job.reply.send(json!({
                    "ok": true,
                    "path": job.path.to_string_lossy(),
                    "width": out_w,
                    "height": out_h
                }));
            }
            false => {
                let _ = job.reply.send(json!({
                    "ok": false,
                    "error": format!("cairo_surface_write_to_png failed with status {write_status}")
                }));
            }
        }
    }

    unsafe extern "C" fn on_pdf_snapshot_ready(
        _source_object: *mut c_void,
        res: *mut c_void,
        user_data: *mut c_void,
    ) {
        let job = Box::from_raw(user_data as *mut PdfJob);
        let st = &mut *job.state_ptr;
        let api = st.api;

        let mut err_ptr: *mut GError = std::ptr::null_mut();
        let raw_surface =
            (api.webkit_web_view_get_snapshot_finish)(st.webview, res, &mut err_ptr);
        match err_ptr.is_null() {
            true => {}
            false => (api.g_error_free)(err_ptr),
        }

        let surface = match raw_surface.is_null() {
            false => raw_surface,
            true => fallback_widget_surface(st),
        };
        match surface.is_null() {
            true => {
                let _ = job.reply.send(json!({
                    "ok": false,
                    "error": "Failed to capture document surface for PDF export"
                }));
                return;
            }
            false => {}
        }

        match job.path.parent() {
            Some(parent) => {
                let _ = std::fs::create_dir_all(parent);
            }
            None => {}
        }

        let img_w = (api.cairo_image_surface_get_width)(surface).max(1) as f64;
        let img_h = (api.cairo_image_surface_get_height)(surface).max(1) as f64;
        // Standard A4 PDF page dimensions in points (595.28 x 841.89 pt)
        let pdf_w_pt = 595.28_f64;
        let pdf_h_pt = 841.89_f64;
        let scale = pdf_w_pt / img_w;
        let slice_h_px = (pdf_h_pt / scale).max(1.0);
        let total_pages = (img_h / slice_h_px).ceil().max(1.0) as usize;

        let c_path = match CString::new(job.path.to_string_lossy().as_bytes()) {
            Ok(p) => p,
            Err(_) => {
                (api.cairo_surface_destroy)(surface);
                let _ = job
                    .reply
                    .send(json!({ "ok": false, "error": "Invalid PDF output path" }));
                return;
            }
        };

        let pdf_surface = (api.cairo_pdf_surface_create)(c_path.as_ptr(), pdf_w_pt, pdf_h_pt);
        let cr = (api.cairo_create)(pdf_surface);
        for page_idx in 0..total_pages {
            let offset_y_px = (page_idx as f64) * slice_h_px;
            (api.cairo_set_source_rgb)(cr, 1.0, 1.0, 1.0);
            (api.cairo_paint)(cr);
            (api.cairo_scale)(cr, scale, scale);
            (api.cairo_set_source_surface)(cr, surface, 0.0, -offset_y_px);
            (api.cairo_paint)(cr);
            // Reset scale before next page
            (api.cairo_scale)(cr, 1.0 / scale, 1.0 / scale);
            (api.cairo_show_page)(cr);
        }
        let status = (api.cairo_status)(cr);
        (api.cairo_destroy)(cr);
        (api.cairo_surface_flush)(pdf_surface);
        (api.cairo_surface_destroy)(pdf_surface);
        (api.cairo_surface_destroy)(surface);
        let _ = libc::malloc_trim(0);

        match status == CAIRO_STATUS_SUCCESS {
            true => {
                let _ = job.reply.send(json!({
                    "ok": true,
                    "path": job.path.to_string_lossy(),
                    "pages": total_pages
                }));
            }
            false => {
                let _ = job.reply.send(json!({
                    "ok": false,
                    "error": format!("Cairo PDF export failed with status {status}")
                }));
            }
        }
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
