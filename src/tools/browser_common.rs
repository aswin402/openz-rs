use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use futures_util::stream::{SplitSink, SplitStream};
use serde_json::Value;
use std::process::Command;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};

pub type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub type WsStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub fn kill_browser_on_port_9222() {
    #[cfg(unix)]
    {
        let _ = Command::new("sh")
            .arg("-c")
            .arg("fuser -k 9222/tcp || kill $(lsof -t -i:9222)")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("cmd")
            .arg("/C")
            .arg("for /f \"tokens=5\" %a in ('netstat -aon ^| findstr 9222') do taskkill /F /PID %a")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

pub async fn ensure_browser_running() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;

    if client.get("http://127.0.0.1:9222/json/list").send().await.is_ok() {
        return Ok(());
    }

    let chrome_paths = ["obscura", "google-chrome", "chrome", "chromium", "chromium-browser"];
    for path in chrome_paths {
        let args = if path == "obscura" {
            vec!["serve", "--port", "9222"]
        } else {
            vec![
                "--headless",
                "--remote-debugging-port=9222",
                "--disable-gpu",
                "--no-sandbox",
                "--disable-dev-shm-usage",
            ]
        };
        if let Ok(child_handle) = Command::new(path)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn() {
            crate::shutdown::register_child(child_handle);
            for _ in 0..25 {
                sleep(Duration::from_millis(200)).await;
                if client.get("http://127.0.0.1:9222/json/list").send().await.is_ok() {
                    return Ok(());
                }
            }
            break;
        }
    }

    if client.get("http://127.0.0.1:9222/json/list").send().await.is_ok() {
        Ok(())
    } else {
        Err(anyhow!("Failed to start any headless browser (obscura, chrome, chromium) on port 9222"))
    }
}

pub async fn send_cdp_cmd(
    write: &mut WsSink,
    read: &mut WsStream,
    message_id: &mut u64,
    method: &str,
    params: Value,
) -> Result<Value> {
    *message_id += 1;
    let id = *message_id;
    let req = serde_json::json!({
        "id": id,
        "method": method,
        "params": params
    });

    write.send(Message::Text(req.to_string())).await?;

    let timeout = Duration::from_secs(30);
    tokio::time::timeout(timeout, async {
        while let Some(msg) = read.next().await {
            let msg = msg?;
            if let Message::Text(text) = msg {
                if let Ok(resp) = serde_json::from_str::<Value>(&text) {
                    if resp.get("id").and_then(|v| v.as_u64()) == Some(id) {
                        return Ok::<Value, anyhow::Error>(resp);
                    }
                }
            }
        }
        Err(anyhow!("Connection closed before receiving response for ID {}", id))
    }).await.map_err(|_| anyhow!("CDP command '{}' timed out after {}s", method, timeout.as_secs()))?
}

pub async fn connect_to_tab(ws_url: &str) -> Result<(WsSink, WsStream)> {
    let (ws_stream, _) = connect_async(ws_url).await?;
    Ok(ws_stream.split())
}
