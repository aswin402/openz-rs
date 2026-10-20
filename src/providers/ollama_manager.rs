use crate::agent::style::colors::{AURA_GOLD, AURA_SLATE, COLOR_RESET, EMERALD_GREEN, ERROR_RED};
use crate::config::{provider_catalog::default_api_base_for_provider, schema::Config};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

struct LocalOllamaState {
    spawned_by_us: bool,
    child: Option<Child>,
}

static LOCAL_OLLAMA: Mutex<LocalOllamaState> = Mutex::new(LocalOllamaState {
    spawned_by_us: false,
    child: None,
});

static ACTIVE_OLLAMA_MODEL: Mutex<Option<String>> = Mutex::new(None);

fn parse_addr(api_base: &str) -> String {
    let without_scheme = api_base
        .strip_prefix("http://")
        .or_else(|| api_base.strip_prefix("https://"))
        .unwrap_or(api_base);
    let host_port = match without_scheme.find('/') {
        Some(idx) => &without_scheme[..idx],
        None => without_scheme,
    };
    host_port.to_string()
}

pub fn is_port_open(addr: &str) -> bool {
    addr.to_socket_addrs()
        .map(| mut addrs| {
            addrs.any(|socket_addr| {
                TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500)).is_ok()
            })
        })
        .unwrap_or(false)
}

fn log_ollama_status(silent: bool, msg: String) {
    match silent {
        true => {}
        false => crate::tui_println!("{}", msg),
    }
}

pub fn ensure_local_ollama(config: &Config) {
    let api_base = config
        .providers
        .ollama
        .as_ref()
        .and_then(|x| x.api_base.clone())
        .unwrap_or_else(|| default_api_base_for_provider("ollama").to_string());

    let addr = parse_addr(&api_base);

    {
        let state = LOCAL_OLLAMA.lock().unwrap_or_else(|e| e.into_inner());
        match is_port_open(&addr) || state.child.is_some() {
            true => return,
            false => {}
        }
    }

    let silent = std::env::var("OPENZ_SILENT").is_ok();
    log_ollama_status(
        silent,
        format!(
            "{}◇ Local Ollama service is not running on {}. Starting it...{}",
            AURA_SLATE, addr, COLOR_RESET
        ),
    );

    match Command::new("ollama")
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            {
                let mut state = LOCAL_OLLAMA.lock().unwrap_or_else(|e| e.into_inner());
                state.spawned_by_us = true;
                state.child = Some(child);
            }

            let mut started = false;
            for _ in 0..60 {
                std::thread::sleep(Duration::from_millis(100));
                match is_port_open(&addr) {
                    true => {
                        started = true;
                        break;
                    }
                    false => {}
                }
            }

            let msg = match started {
                true => format!(
                    "{}✓ Local Ollama service started successfully.{}",
                    EMERALD_GREEN, COLOR_RESET
                ),
                false => format!(
                    "{}✕ Local Ollama service started but failed to bind within timeout.{}",
                    AURA_GOLD, COLOR_RESET
                ),
            };
            log_ollama_status(silent, msg);
        }
        Err(e) => {
            let msg = match e.kind() {
                std::io::ErrorKind::NotFound => format!(
                    "{}✕ 'ollama' executable not found in PATH. Please install Ollama or start it manually.{}",
                    ERROR_RED, COLOR_RESET
                ),
                _ => format!(
                    "{}✕ Failed to spawn 'ollama serve': {}{}",
                    ERROR_RED, e, COLOR_RESET
                ),
            };
            log_ollama_status(silent, msg);
        }
    }
}

pub fn stop_local_ollama() {
    let mut state = LOCAL_OLLAMA.lock().unwrap_or_else(|e| e.into_inner());
    match state.spawned_by_us {
        false => {}
        true => {
            match state.child.take() {
                Some(mut child) => {
                    let silent = std::env::var("OPENZ_SILENT").is_ok();
                    log_ollama_status(
                        silent,
                        format!(
                            "{}◇ Stopping local Ollama service...{}",
                            AURA_SLATE, COLOR_RESET
                        ),
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                }
                None => {}
            }
            state.spawned_by_us = false;
        }
    }
}

pub fn get_active_ollama_model() -> Option<String> {
    let guard = ACTIVE_OLLAMA_MODEL
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard.clone()
}

pub fn set_active_ollama_model(model: Option<String>) {
    let mut guard = ACTIVE_OLLAMA_MODEL
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *guard = model;
}

fn ollama_native_generate_url(api_base: &str) -> String {
    match api_base
        .strip_suffix("/v1")
        .or_else(|| api_base.strip_suffix("/v1/"))
    {
        Some(prefix) => format!("{prefix}/generate"),
        None => format!("{api_base}/api/generate"),
    }
}

pub async fn unload_ollama_model(config: &Config, model: &str) {
    let api_base = config
        .providers
        .ollama
        .as_ref()
        .and_then(|x| x.api_base.clone())
        .unwrap_or_else(|| default_api_base_for_provider("ollama").to_string());

    let native_url = ollama_native_generate_url(&api_base);

    let client = crate::core::http::http_client_with_timeout(Duration::from_secs(3));

    let payload = serde_json::json!({
        "model": model,
        "keep_alive": 0
    });

    let _ = client.post(&native_url).json(&payload).send().await;
}

pub async fn unload_active_ollama_model(config: &Config) {
    match get_active_ollama_model() {
        Some(model) => unload_ollama_model(config, &model).await,
        None => {}
    }
}
