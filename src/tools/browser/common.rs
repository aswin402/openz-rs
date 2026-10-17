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

pub(crate) const URL_ALIASES: &[&str] = &[
    "url",
    "target_url",
    "targetUrl",
    "uri",
    "link",
    "target",
    "site",
    "website",
    "domain",
    "host",
    "page",
    "endpoint",
    "href",
    "address",
];

pub(crate) const ACTION_ALIASES: &[&str] = &["action", "act", "command", "cmd", "method"];

pub(crate) const SELECTOR_ALIASES: &[&str] = &[
    "selector",
    "css_selector",
    "cssSelector",
    "css",
    "element",
    "query",
    "target",
];

pub(crate) const REF_ID_ALIASES: &[&str] = &[
    "ref_id",
    "refId",
    "ref",
    "element",
    "id",
    "target_ref",
    "targetRef",
];

pub(crate) const TEXT_ALIASES: &[&str] = &["text", "value", "query", "content", "input", "string"];

pub(crate) const PATH_ALIASES: &[&str] = &[
    "path",
    "output",
    "output_path",
    "outputPath",
    "file_path",
    "file",
    "dest",
];

pub(crate) const SCRIPT_ALIASES: &[&str] = &["script", "expression", "code", "js", "expr"];

pub(crate) fn extract_first_str<'a>(arguments: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|&key| arguments.get(key).and_then(|v| v.as_str()))
}

pub(crate) fn extract_trimmed_non_empty_str<'a>(
    arguments: &'a Value,
    keys: &[&str],
) -> Option<&'a str> {
    extract_first_str(arguments, keys)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub fn browser_cdp_port() -> u16 {
    crate::config::loader::load_config()
        .map(|c| c.browser.cdp_port)
        .unwrap_or(9222)
}

pub fn kill_browser_on_port(port: u16) {
    #[cfg(unix)]
    let cmd = format!(
        "for pid in $(lsof -t -i:{port} 2>/dev/null); do \
            cmd=$(ps -p $pid -o comm= 2>/dev/null); \
            case \"$cmd\" in *chrome*|*chromium*|*obscura*) kill -9 $pid 2>/dev/null ;; esac; \
        done"
    );
    #[cfg(windows)]
    let cmd = format!(
        "powershell -NoProfile -Command \"Get-NetTCPConnection -LocalPort {port} -ErrorAction SilentlyContinue | ForEach-Object {{ Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue }} | Where-Object {{ $_.ProcessName -match 'chrome|chromium|obscura' }} | Stop-Process -Force\""
    );

    let _ = crate::core::process::host_shell_command(&cmd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
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

    match client.get(&list_url).send().await.is_ok() {
        true => return Ok(()),
        false => {}
    }

    let chrome_paths = [
        "google-chrome",
        "chrome",
        "chromium",
        "chromium-browser",
        "obscura",
    ];
    let port_str = port.to_string();
    let port_arg = format!("--remote-debugging-port={port}");
    for path in chrome_paths {
        let args: Vec<&str> = match path {
            "obscura" => vec!["serve", "--port", &port_str],
            _ => vec![
                "--headless",
                &port_arg,
                "--disable-gpu",
                "--no-sandbox",
                "--disable-dev-shm-usage",
            ],
        };
        match Command::new(path)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child_handle) => {
                crate::shutdown::register_child_with_metadata(
                    child_handle,
                    format!("{} {}", path, args.join(" ")),
                    "browser_daemon",
                );
                for _ in 0..25 {
                    sleep(Duration::from_millis(200)).await;
                    match client.get(&list_url).send().await.is_ok() {
                        true => return Ok(()),
                        false => {}
                    }
                }
                break;
            }
            Err(_) => {}
        }
    }

    client
        .get(&list_url)
        .send()
        .await
        .is_ok()
        .then_some(())
        .ok_or_else(|| {
            anyhow!("Failed to start any headless browser (obscura, chrome, chromium) on port {port}")
        })
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
            let matched = match msg {
                Message::Text(text) => serde_json::from_str::<Value>(&text)
                    .ok()
                    .filter(|resp| resp.get("id").and_then(|v| v.as_u64()) == Some(id)),
                _ => None,
            };
            match matched {
                Some(resp) => return Ok::<Value, anyhow::Error>(resp),
                None => {}
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

pub(crate) async fn request_new_cdp_tab(
    client: &reqwest::Client,
    new_tab_url: &str,
) -> std::result::Result<reqwest::Response, reqwest::Error> {
    let put_res = client.put(new_tab_url).send().await;
    match put_res.as_ref().is_ok_and(|r| r.status().is_success()) {
        true => put_res,
        false => client.get(new_tab_url).send().await,
    }
}

pub async fn obtain_tab_websocket_url(cdp_port: u16) -> Result<(String, String)> {
    let client = crate::core::http::default_http_client();
    let new_tab_url = format!("http://127.0.0.1:{cdp_port}/json/new");

    let res = request_new_cdp_tab(&client, &new_tab_url).await;

    match res.ok().filter(|r| r.status().is_success()) {
        Some(r) => match r.json::<Value>().await.ok().and_then(|tab_info| {
            let tid = tab_info
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("page-1")
                .to_string();
            tab_info
                .get("webSocketDebuggerUrl")
                .and_then(|v| v.as_str())
                .map(|ws| (tid, ws.to_string()))
        }) {
            Some((tid, ws_url)) => return Ok((tid, ws_url)),
            None => {}
        },
        None => {}
    }

    // Fallback: /json/list (e.g. for Obscura or existing browser daemon tabs)
    let list_url = format!("http://127.0.0.1:{cdp_port}/json/list");
    let list_res = client.get(&list_url).send().await?;
    let tabs: Vec<Value> = list_res.json().await?;
    let tab = tabs
        .into_iter()
        .find(|t| t.get("webSocketDebuggerUrl").and_then(|v| v.as_str()).is_some())
        .ok_or_else(|| anyhow!("No available tab with webSocketDebuggerUrl found in /json/list"))?;
    let tid = tab
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("page-1")
        .to_string();
    let ws_url = tab
        .get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    Ok((tid, ws_url))
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;
