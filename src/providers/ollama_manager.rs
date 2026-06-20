use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use crate::config::schema::Config;
use crate::agent::style::colors::{AURA_SLATE, EMERALD_GREEN, ERROR_RED, COLOR_RESET, AURA_GOLD};

static LOCAL_OLLAMA_CHILD: Mutex<Option<Child>> = Mutex::new(None);
static SPAWNED_BY_US: Mutex<bool> = Mutex::new(false);
static ACTIVE_OLLAMA_MODEL: Mutex<Option<String>> = Mutex::new(None);

fn parse_addr(api_base: &str) -> String {
    let without_scheme = if let Some(stripped) = api_base.strip_prefix("http://") {
        stripped
    } else if let Some(stripped) = api_base.strip_prefix("https://") {
        stripped
    } else {
        api_base
    };
    let host_port = match without_scheme.find('/') {
        Some(idx) => &without_scheme[..idx],
        None => without_scheme,
    };
    host_port.to_string()
}

pub fn is_port_open(addr: &str) -> bool {
    if let Ok(addrs) = addr.to_socket_addrs() {
        for socket_addr in addrs {
            if TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500)).is_ok() {
                return true;
            }
        }
    }
    false
}

pub fn ensure_local_ollama(config: &Config) {
    let api_base = config
        .providers
        .ollama
        .as_ref()
        .and_then(|x| x.api_base.clone())
        .unwrap_or_else(|| "http://localhost:11434/v1".to_string());

    let addr = parse_addr(&api_base);

    // Atomic check: verify port is open OR we already have a child running
    {
        let child_guard = LOCAL_OLLAMA_CHILD.lock().unwrap_or_else(|e| e.into_inner());
        if is_port_open(&addr) || child_guard.is_some() {
            return;
        }
    }

    let silent = std::env::var("OPENZ_SILENT").is_ok();

    if !silent {
        crate::tui_println!(
            "{}◇ Local Ollama service is not running on {}. Starting it...{}",
            AURA_SLATE,
            addr,
            COLOR_RESET
        );
    }

    // Spawn "ollama serve"
    match Command::new("ollama")
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            if let Ok(mut spawned_guard) = SPAWNED_BY_US.lock() {
                *spawned_guard = true;
            }
            if let Ok(mut child_guard) = LOCAL_OLLAMA_CHILD.lock() {
                *child_guard = Some(child);
            }

            // Poll until it starts listening or we timeout (up to 6 seconds)
            let mut started = false;
            for _ in 0..60 {
                std::thread::sleep(Duration::from_millis(100));
                if is_port_open(&addr) {
                    started = true;
                    break;
                }
            }

            if started {
                if !silent {
                    crate::tui_println!(
                        "{}✓ Local Ollama service started successfully.{}",
                        EMERALD_GREEN,
                        COLOR_RESET
                    );
                }
            } else {
                if !silent {
                    crate::tui_println!(
                        "{}✕ Local Ollama service started but failed to bind within timeout.{}",
                        AURA_GOLD,
                        COLOR_RESET
                    );
                }
            }
        }
        Err(e) => {
            if !silent {
                if e.kind() == std::io::ErrorKind::NotFound {
                    crate::tui_println!(
                        "{}✕ 'ollama' executable not found in PATH. Please install Ollama or start it manually.{}",
                        ERROR_RED,
                        COLOR_RESET
                    );
                } else {
                    crate::tui_println!(
                        "{}✕ Failed to spawn 'ollama serve': {}{}",
                        ERROR_RED,
                        e,
                        COLOR_RESET
                    );
                }
            }
        }
    }
}

pub fn stop_local_ollama() {
    let mut should_kill = false;
    if let Ok(spawned_guard) = SPAWNED_BY_US.lock() {
        if *spawned_guard {
            should_kill = true;
        }
    }

    if should_kill {
        if let Ok(mut child_guard) = LOCAL_OLLAMA_CHILD.lock() {
            if let Some(mut child) = child_guard.take() {
                let silent = std::env::var("OPENZ_SILENT").is_ok();
                if !silent {
                    crate::tui_println!(
                        "{}◇ Stopping local Ollama service...{}",
                        AURA_SLATE,
                        COLOR_RESET
                    );
                }
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Ok(mut spawned_guard) = SPAWNED_BY_US.lock() {
            *spawned_guard = false;
        }
    }
}

pub fn get_active_ollama_model() -> Option<String> {
    if let Ok(guard) = ACTIVE_OLLAMA_MODEL.lock() {
        guard.clone()
    } else {
        None
    }
}

pub fn set_active_ollama_model(model: Option<String>) {
    if let Ok(mut guard) = ACTIVE_OLLAMA_MODEL.lock() {
        *guard = model;
    }
}

pub async fn unload_ollama_model(config: &Config, model: &str) {
    let api_base = config
        .providers
        .ollama
        .as_ref()
        .and_then(|x| x.api_base.clone())
        .unwrap_or_else(|| "http://localhost:11434/v1".to_string());

    // Send keep_alive: 0 to /api/generate or /api/chat. Ollama's native port for generate is usually localhost:11434
    // If it's http://localhost:11434/v1, native generate endpoint is http://localhost:11434/api/generate
    let native_url = if api_base.ends_with("/v1") {
        format!("{}/generate", &api_base[..api_base.len() - 3])
    } else if api_base.ends_with("/v1/") {
        format!("{}/generate", &api_base[..api_base.len() - 4])
    } else {
        format!("{}/api/generate", api_base)
    };

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    let payload = serde_json::json!({
        "model": model,
        "keep_alive": 0
    });

    let _ = client.post(&native_url).json(&payload).send().await;
}

pub async fn unload_active_ollama_model(config: &Config) {
    let active = get_active_ollama_model();
    if let Some(model) = active {
        unload_ollama_model(config, &model).await;
    }
}
