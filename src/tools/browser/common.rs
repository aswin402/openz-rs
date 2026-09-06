use anyhow::{anyhow, Result};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::process::Command;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};

pub type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub type WsStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub fn browser_cdp_port() -> u16 {
    crate::config::loader::load_config()
        .map(|c| c.browser.cdp_port)
        .unwrap_or(9222)
}

pub fn kill_browser_on_port(port: u16) {
    #[cfg(unix)]
    {
        // Target only known headless browser binaries on the port to avoid killing unrelated developer tools
        let cmd = format!(
            "for pid in $(lsof -t -i:{port} 2>/dev/null); do \
                cmd=$(ps -p $pid -o comm= 2>/dev/null); \
                case \"$cmd\" in *chrome*|*chromium*|*obscura*) kill -9 $pid 2>/dev/null ;; esac; \
            done"
        );
        let _ = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let cmd = format!(
            "powershell -NoProfile -Command \"Get-NetTCPConnection -LocalPort {port} -ErrorAction SilentlyContinue | ForEach-Object {{ Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue }} | Where-Object {{ $_.ProcessName -match 'chrome|chromium|obscura' }} | Stop-Process -Force\""
        );
        let _ = Command::new("cmd")
            .arg("/C")
            .arg(&cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

pub fn kill_browser_on_cdp_port() {
    kill_browser_on_port(browser_cdp_port());
}

pub fn kill_browser_on_port_9222() {
    kill_browser_on_cdp_port();
}

pub async fn ensure_browser_running() -> Result<()> {
    let port = browser_cdp_port();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()?;

    let list_url = format!("http://127.0.0.1:{port}/json/list");

    if client
        .get(&list_url)
        .send()
        .await
        .is_ok()
    {
        return Ok(());
    }

    let chrome_paths = [
        "obscura",
        "google-chrome",
        "chrome",
        "chromium",
        "chromium-browser",
    ];
    let port_str = port.to_string();
    let port_arg = format!("--remote-debugging-port={port}");
    for path in chrome_paths {
        let args = if path == "obscura" {
            vec!["serve", "--port", &port_str]
        } else {
            vec![
                "--headless",
                &port_arg,
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
            .spawn()
        {
            crate::shutdown::register_child_with_metadata(
                child_handle,
                format!("{} {}", path, args.join(" ")),
                "browser_daemon",
            );
            for _ in 0..25 {
                sleep(Duration::from_millis(200)).await;
                if client
                    .get(&list_url)
                    .send()
                    .await
                    .is_ok()
                {
                    return Ok(());
                }
            }
            break;
        }
    }

    if client
        .get(&list_url)
        .send()
        .await
        .is_ok()
    {
        Ok(())
    } else {
        Err(anyhow!(
            "Failed to start any headless browser (obscura, chrome, chromium) on port {port}"
        ))
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
        Err(anyhow!(
            "Connection closed before receiving response for ID {}",
            id
        ))
    })
    .await
    .map_err(|_| {
        anyhow!(
            "CDP command '{}' timed out after {}s",
            method,
            timeout.as_secs()
        )
    })?
}

pub async fn connect_to_tab(ws_url: &str) -> Result<(WsSink, WsStream)> {
    let (ws_stream, _) = connect_async(ws_url).await?;
    Ok(ws_stream.split())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_cdp_port_default() {
        let port = browser_cdp_port();
        assert!(port > 0);
    }

    #[test]
    fn test_kill_browser_on_port_non_existent() {
        // Port 59999 should safely execute without error or panics
        kill_browser_on_port(59999);
    }
}

