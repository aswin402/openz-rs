//! Unified Native OpenZ `browser` tool.
//!
//! Consolidates `obscura_browser`, `gsd_browser`, `firefox_browser`, and `inspect_browsers`
//! into a single zero-external-dependency native WebKit2GTK + PageController tool supporting
//! both headless (`GtkOffscreenWindow`) and GUI (`GtkWindow`) execution.

use super::client::{BrowserSessionClient, ScreenshotRequest, SessionLaunchSpec};
use super::common::{
    extract_trimmed_non_empty_str, ACTION_ALIASES, PATH_ALIASES, REF_ID_ALIASES, SCRIPT_ALIASES,
    SELECTOR_ALIASES, TEXT_ALIASES, URL_ALIASES,
};
use super::status::collect_browser_status_report;
use super::worker::{normalize_cli_url, parse_size_spec, sanitize_session_name};
use crate::config::resolve_path;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::net::IpAddr;
use std::time::Duration;

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeBrowserTool;

impl NativeBrowserTool {
    pub fn new() -> Self {
        Self
    }
}

pub fn extract_browser_url(arguments: &Value) -> Option<String> {
    match arguments {
        Value::String(raw) => {
            let trimmed = raw.trim();
            (!trimmed.is_empty()).then(|| normalize_cli_url(trimmed))
        }
        Value::Object(_) => extract_trimmed_non_empty_str(arguments, URL_ALIASES).map(normalize_cli_url),
        _ => None,
    }
}

pub fn resolve_canonical_browser_action(arguments: &Value) -> String {
    match arguments {
        Value::String(_) => "render".to_string(),
        Value::Object(map) => {
            let raw = extract_trimmed_non_empty_str(arguments, ACTION_ALIASES);
            match raw {
                Some(act) => normalize_action_name(act),
                None => {
                    let has_script = extract_trimmed_non_empty_str(arguments, SCRIPT_ALIASES).is_some();
                    let has_url = extract_trimmed_non_empty_str(arguments, URL_ALIASES).is_some();
                    let has_html = map
                        .get("html")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| !s.trim().is_empty());
                    match (has_script, has_url || has_html, map.is_empty()) {
                        (true, _, _) => "eval_js".to_string(),
                        (false, true, _) => "navigate".to_string(),
                        (false, false, true) => "status".to_string(),
                        (false, false, false) => "ui_tree".to_string(),
                    }
                }
            }
        }
        _ => "status".to_string(),
    }
}

pub fn normalize_action_name(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "goto" | "open" | "visit" | "load" | "navigate" => "navigate".to_string(),
        "render" | "markdown" | "extract" | "read" | "scrape" => "render".to_string(),
        "html" | "outer_html" | "page_source" | "source" => "html".to_string(),
        "text" | "inner_text" => "text".to_string(),
        "ui_tree" | "snapshot" | "a11y" | "dom" | "tree" | "elements" => "ui_tree".to_string(),
        "click" | "tap" => "click".to_string(),
        "type" | "fill" | "input" | "enter_text" => "type".to_string(),
        "fill_form" | "form" => "fill_form".to_string(),
        "select_option" | "select" | "choose" => "select_option".to_string(),
        "hover" | "mouseover" => "hover".to_string(),
        "scroll" | "wheel" => "scroll".to_string(),
        "press_key" | "press" | "key" => "press_key".to_string(),
        "wait_for" | "wait" => "wait_for".to_string(),
        "eval_js" | "eval" | "evaluate" | "js" | "execute_script" | "script" => {
            "eval_js".to_string()
        }
        "screenshot" | "capture" | "snap" => "screenshot".to_string(),
        "save_pdf" | "pdf" | "export_pdf" | "print_pdf" => "save_pdf".to_string(),
        "back" | "go_back" => "back".to_string(),
        "forward" | "go_forward" => "forward".to_string(),
        "reload" | "refresh" => "reload".to_string(),
        "resize" | "viewport" | "set_viewport" => "resize".to_string(),
        "mode" | "set_mode" | "show" | "hide" => "mode".to_string(),
        "logs" | "console" | "errors" => "logs".to_string(),
        "network" | "requests" => "network".to_string(),
        "inspect_overlay" | "marks" | "som" | "annotate" => "inspect_overlay".to_string(),
        "takeover" | "takeover_mask" | "human_takeover" | "pause" => "takeover_mask".to_string(),
        "status" | "health" | "inspect" | "inspect_browsers" | "list_sessions" => {
            "status".to_string()
        }
        "close" | "stop" | "quit" => "close".to_string(),
        other => other.to_string(),
    }
}

pub fn validate_browser_url_target(raw_url: &str, allow_lan: bool) -> Result<()> {
    let trimmed = raw_url.trim();
    let is_in_memory = trimmed.starts_with("about:") || trimmed.starts_with("data:text/html");
    match is_in_memory {
        true => return Ok(()),
        false => {}
    }

    match trimmed.strip_prefix("file://") {
        Some(file_path) => {
            let lower = file_path.to_ascii_lowercase();
            let sensitive = [
                "/.ssh/",
                "/etc/shadow",
                "/etc/gshadow",
                ".openz/config.json",
                "/.aws/",
                "/.gnupg/",
                "/.env",
            ]
            .iter()
            .any(|pat| lower.contains(pat) || lower.ends_with("/.env"));
            return match sensitive {
                true => Err(anyhow!(
                    "Blocked navigation to sensitive local file path: {file_path}"
                )),
                false => Ok(()),
            };
        }
        None => {}
    }

    let parsed = url::Url::parse(trimmed).map_err(|e| anyhow!("Invalid browser URL '{trimmed}': {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(anyhow!(
                "Unsupported browser URL scheme '{other}' (expected http, https, file, or data:text/html)"
            ));
        }
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("Browser URL '{trimmed}' is missing a host"))?
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();

    let is_cloud_metadata = matches!(
        host.as_str(),
        "169.254.169.254"
            | "100.100.100.200"
            | "metadata.google.internal"
            | "metadata.goog"
            | "instance-data"
    ) || host.ends_with(".internal");
    match is_cloud_metadata {
        true => {
            return Err(anyhow!(
                "SSRF blocked: cloud metadata endpoint '{host}' is never accessible"
            ));
        }
        false => {}
    }

    let is_local_dev = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1")
        || host.ends_with(".localhost");
    match is_local_dev {
        true => return Ok(()),
        false => {}
    }

    match host.parse::<IpAddr>() {
        Ok(ip) => {
            let is_link_local = match ip {
                IpAddr::V4(v4) => v4.is_link_local() || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1])),
                IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) == 0xfe80,
            };
            match is_link_local {
                true => Err(anyhow!("SSRF blocked: link-local/CGNAT address {ip} is forbidden")),
                false => match allow_lan || crate::tools::searchxyz::core::crawler::ssrf::is_safe_ip(&ip) {
                    true => Ok(()),
                    false => Err(anyhow!(
                        "SSRF blocked: private LAN IP {ip} requires allow_local=true"
                    )),
                },
            }
        }
        Err(_) => Ok(()),
    }
}

fn extract_session_spec(arguments: &Value, action: &str) -> SessionLaunchSpec {
    let mut spec = SessionLaunchSpec::default();
    match arguments.get("session").and_then(|v| v.as_str()) {
        Some(s) => spec.session = sanitize_session_name(s),
        None => {}
    }
    match arguments.get("profile").and_then(|v| v.as_str()) {
        Some(p) => {
            let clean = sanitize_session_name(p);
            spec.profile = (!matches!(clean.as_str(), "ephemeral" | "none")).then_some(clean);
        }
        None => {}
    }
    match arguments.get("mode").and_then(|v| v.as_str()) {
        Some(m) => {
            spec.gui = matches!(m.trim().to_ascii_lowercase().as_str(), "gui" | "visible" | "headed");
        }
        None => match arguments.get("gui").and_then(|v| v.as_bool()) {
            Some(flag) => spec.gui = flag,
            None => match arguments.get("headless").and_then(|v| v.as_bool()) {
                Some(hl) => spec.gui = !hl,
                None => {
                    let is_show = action == "show";
                    match is_show {
                        true => spec.gui = true,
                        false => {}
                    }
                }
            },
        },
    }
    match arguments
        .get("size")
        .and_then(|v| v.as_str())
        .and_then(parse_size_spec)
    {
        Some((w, h)) => {
            spec.width = w;
            spec.height = h;
        }
        None => {}
    }
    match arguments.get("width").and_then(|v| v.as_i64()) {
        Some(w) => spec.width = (w as i32).clamp(120, 7680),
        None => {}
    }
    match arguments.get("height").and_then(|v| v.as_i64()) {
        Some(h) => spec.height = (h as i32).clamp(120, 7680),
        None => {}
    }
    match arguments.get("memory_limit_mb").and_then(|v| v.as_u64()) {
        Some(mb) => spec.memory_limit_mb = Some((mb as u32).clamp(48, 2048)),
        None => {}
    }
    spec
}

fn parse_element_index(arguments: &Value) -> Option<i64> {
    arguments
        .get("index")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            arguments
                .get("index")
                .and_then(|v| v.as_str())
                .and_then(parse_numeric_ref)
        })
        .or_else(|| {
            extract_trimmed_non_empty_str(arguments, REF_ID_ALIASES).and_then(parse_numeric_ref)
        })
}

fn parse_numeric_ref(raw: &str) -> Option<i64> {
    let trimmed = raw
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_start_matches("@e")
        .trim_start_matches("@E")
        .trim();
    (!trimmed.is_empty() && trimmed.bytes().all(|b| b.is_ascii_digit()))
        .then(|| trimmed.parse::<i64>().ok())
        .flatten()
}

#[async_trait::async_trait]
impl Tool for NativeBrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Control OpenZ's native low-RAM WebKit2GTK browser in headless (default, zero popup) or GUI mode (`mode=\"gui\"`). Supports live DOM/Shadow-DOM/iframe indexed `ui_tree` (`[0]`, `[1]`), differential `ui_diff` on `click`/`type`/`fill_form`/`select_option`/`scroll`, clean JS-rendered Markdown extraction (`render`), async `eval_js`, Set-of-Mark `screenshot`, multi-page `save_pdf`, `network`/`logs` ring buffers, Human Takeover bar, and runtime `status`."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate",
                        "ui_tree",
                        "render",
                        "html",
                        "text",
                        "click",
                        "type",
                        "fill_form",
                        "select_option",
                        "hover",
                        "scroll",
                        "press_key",
                        "wait_for",
                        "eval_js",
                        "screenshot",
                        "save_pdf",
                        "back",
                        "forward",
                        "reload",
                        "resize",
                        "mode",
                        "logs",
                        "network",
                        "inspect_overlay",
                        "takeover_mask",
                        "status",
                        "close"
                    ],
                    "description": "Browser operation. Defaults to 'navigate' when url is supplied, 'eval_js' when script is supplied, or 'status' when empty."
                },
                "url": {
                    "type": "string",
                    "description": "Target URL (http/https, localhost dev server, or local HTML file path)."
                },
                "mode": {
                    "type": "string",
                    "enum": ["headless", "gui"],
                    "description": "Execution mode: 'headless' (default offscreen surface, 0 popup) or 'gui' (live OpenZ Browser window with Human Takeover toggle)."
                },
                "session": {
                    "type": "string",
                    "description": "Named browser session (default: 'default'). Multiple sessions stay isolated."
                },
                "profile": {
                    "type": "string",
                    "description": "Optional persistent cookie/storage profile name under ~/.openz/browser_profiles/<profile> (omit for clean ephemeral session)."
                },
                "index": {
                    "type": "integer",
                    "description": "Interactive element index `[0]`, `[1]` from `ui_tree` for click, type, select_option, or hover."
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector target for click, type, wait_for, scroll, or cropped screenshot."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type/fill, visible button/link label to click, or text to wait_for."
                },
                "submit": {
                    "type": "boolean",
                    "description": "Submit parent form / press Enter after typing or fill_form."
                },
                "fields": {
                    "type": "array",
                    "description": "Batch form fields for fill_form: [{ index, selector, label, value }].",
                    "items": { "type": "object" }
                },
                "value": {
                    "type": "string",
                    "description": "Value or option label for select_option / type."
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript expression or async body for eval_js (Promises are automatically awaited)."
                },
                "query": {
                    "type": "string",
                    "description": "Optional keyword filter to prune `ui_tree` to matching interactive elements."
                },
                "role": {
                    "type": "string",
                    "description": "Optional role/tag filter for `ui_tree` (e.g. 'button', 'link', 'textbox', 'combobox')."
                },
                "diff_only": {
                    "type": "boolean",
                    "description": "Return only `[+]`/`[~]`/`[-]` differential changes since the last `ui_tree`."
                },
                "direction": {
                    "type": "string",
                    "enum": ["down", "up", "left", "right", "top", "bottom"],
                    "description": "Scroll direction for `scroll` (default: 'down')."
                },
                "scroll_id": {
                    "type": "string",
                    "description": "Optional inner scrollable container ID (`scroll#0`) reported by `ui_tree`."
                },
                "path": {
                    "type": "string",
                    "description": "Output file path for `screenshot` (.png) or `save_pdf` (.pdf)."
                },
                "annotated": {
                    "type": "boolean",
                    "description": "Draw `[0]`, `[1]` Set-of-Mark badges on `screenshot` so visual coordinates match `index`."
                },
                "full_page": {
                    "type": "boolean",
                    "description": "Capture full scrolling document height on `screenshot`."
                },
                "size": {
                    "type": "string",
                    "description": "Viewport preset ('desktop', 'tablet', 'mobile', '1080p') or 'WIDTHxHEIGHT'."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds for navigation, wait_for, or eval_js (default: 15)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = resolve_canonical_browser_action(arguments);

        match action.as_str() {
            "status" => return collect_browser_status_report().await,
            _ => {}
        }

        let spec = extract_session_spec(arguments, &action);
        match action.as_str() {
            "close" => {
                let sock = super::worker::session_socket_path(&spec.session);
                match sock.exists() {
                    true => {
                        let client = BrowserSessionClient::ensure_session(&spec).await.ok();
                        match client {
                            Some(c) => {
                                let _ = c.close().await;
                            }
                            None => {
                                let _ = std::fs::remove_file(&sock);
                            }
                        }
                    }
                    false => {}
                }
                return Ok(json!({
                    "status": "success",
                    "action": "close",
                    "session": spec.session,
                    "closed": true
                }));
            }
            _ => {}
        }

        let url_opt = extract_browser_url(arguments);
        let allow_lan = arguments
            .get("allow_local")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        match url_opt.as_deref() {
            Some(u) => validate_browser_url_target(u, allow_lan)?,
            None => {}
        }

        let timeout_secs = arguments
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(15)
            .clamp(2, 60);
        let timeout_ms = timeout_secs * 1000;
        let settle_ms = arguments
            .get("settle_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(180)
            .min(5_000);

        let client = BrowserSessionClient::ensure_session(&spec).await?;

        // Navigate first when a URL or inline HTML was passed alongside an action
        let inline_html = arguments
            .get("html")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty());
        match (url_opt.as_deref(), inline_html) {
            (Some(u), _) => {
                client.navigate_url(u, timeout_ms, settle_ms).await?;
            }
            (None, Some(html_doc)) => {
                client.load_html(html_doc, None, settle_ms).await?;
            }
            (None, None) => match action == "navigate" {
                true => {
                    return Err(anyhow!(
                        "browser action='navigate' requires a 'url' or 'html' parameter"
                    ));
                }
                false => {}
            },
        }

        match action.as_str() {
            "navigate" => {
                let format_pref = arguments
                    .get("format")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ui_tree");
                match format_pref.eq_ignore_ascii_case("markdown") {
                    true => {
                        let rendered = client
                            .web_agent(
                                json!({ "action": "render", "format": "markdown" }),
                                Duration::from_secs(timeout_secs),
                            )
                            .await?;
                        Ok(json!({
                            "status": "success",
                            "engine": "openz_webkit2gtk",
                            "session": spec.session,
                            "mode": match spec.gui { true => "gui", false => "headless" },
                            "url": rendered.get("url").cloned().unwrap_or(Value::Null),
                            "title": rendered.get("title").cloned().unwrap_or(Value::Null),
                            "output": rendered.get("output").cloned().unwrap_or(Value::Null),
                        }))
                    }
                    false => {
                        let snap = client
                            .web_agent(
                                json!({
                                    "action": "ui_tree",
                                    "query": arguments.get("query"),
                                    "role": arguments.get("role"),
                                    "max_elements": arguments.get("max_elements").and_then(|v| v.as_u64()).unwrap_or(150),
                                }),
                                Duration::from_secs(timeout_secs),
                            )
                            .await?;
                        Ok(json!({
                            "status": "success",
                            "engine": "openz_webkit2gtk",
                            "session": spec.session,
                            "mode": match spec.gui { true => "gui", false => "headless" },
                            "url": snap.get("url").cloned().unwrap_or(Value::Null),
                            "title": snap.get("title").cloned().unwrap_or(Value::Null),
                            "viewport": snap.get("viewport").cloned().unwrap_or(Value::Null),
                            "element_count": snap.get("element_count").cloned().unwrap_or(Value::Null),
                            "ui_tree": snap.get("tree_text").cloned().unwrap_or(Value::Null),
                            "scrollable_containers": snap.get("scrollable_containers").cloned().unwrap_or(Value::Null),
                        }))
                    }
                }
            }
            "render" | "html" | "text" => {
                let fmt = match action.as_str() {
                    "html" => "html",
                    "text" => "text",
                    _ => arguments
                        .get("format")
                        .and_then(|v| v.as_str())
                        .unwrap_or("markdown"),
                };
                let data = client
                    .web_agent(
                        json!({ "action": "render", "format": fmt }),
                        Duration::from_secs(timeout_secs),
                    )
                    .await?;
                Ok(json!({
                    "status": "success",
                    "engine": "openz_webkit2gtk",
                    "action": action,
                    "format": fmt,
                    "url": data.get("url").cloned().unwrap_or(Value::Null),
                    "final_url": data.get("final_url").cloned().unwrap_or(Value::Null),
                    "title": data.get("title").cloned().unwrap_or(Value::Null),
                    "output": data.get("output").cloned().unwrap_or(Value::Null),
                }))
            }
            "ui_tree" => {
                let data = client
                    .web_agent(
                        json!({
                            "action": "ui_tree",
                            "query": arguments.get("query"),
                            "role": arguments.get("role"),
                            "diff_only": arguments.get("diff_only").and_then(|v| v.as_bool()).unwrap_or(false),
                            "viewport_only": arguments.get("viewport_only").and_then(|v| v.as_bool()).unwrap_or(false),
                            "max_elements": arguments.get("max_elements").and_then(|v| v.as_u64()).unwrap_or(180),
                        }),
                        Duration::from_secs(timeout_secs),
                    )
                    .await?;
                Ok(json!({
                    "status": "success",
                    "engine": "openz_webkit2gtk",
                    "session": spec.session,
                    "data": data,
                    "output": data.get("tree_text").or_else(|| data.get("ui_diff")).cloned().unwrap_or(Value::Null),
                }))
            }
            "click" => {
                let payload = json!({
                    "action": "click",
                    "index": parse_element_index(arguments),
                    "selector": extract_trimmed_non_empty_str(arguments, SELECTOR_ALIASES),
                    "text": extract_trimmed_non_empty_str(arguments, TEXT_ALIASES),
                    "label": arguments.get("label").and_then(|v| v.as_str()),
                    "x": arguments.get("x").and_then(|v| v.as_f64()),
                    "y": arguments.get("y").and_then(|v| v.as_f64()),
                });
                let data = client
                    .web_agent(payload, Duration::from_secs(timeout_secs))
                    .await?;
                Ok(json!({ "status": "success", "action": "click", "result": data }))
            }
            "type" => {
                let payload = json!({
                    "action": "type",
                    "index": parse_element_index(arguments),
                    "selector": extract_trimmed_non_empty_str(arguments, SELECTOR_ALIASES),
                    "label": arguments.get("label").and_then(|v| v.as_str()),
                    "text": extract_trimmed_non_empty_str(arguments, TEXT_ALIASES).unwrap_or(""),
                    "submit": arguments.get("submit").and_then(|v| v.as_bool()).unwrap_or(false),
                });
                let data = client
                    .web_agent(payload, Duration::from_secs(timeout_secs))
                    .await?;
                Ok(json!({ "status": "success", "action": "type", "result": data }))
            }
            "fill_form" => {
                let payload = json!({
                    "action": "fill_form",
                    "fields": arguments.get("fields").cloned().unwrap_or_else(|| json!([])),
                    "submit": arguments.get("submit").and_then(|v| v.as_bool()).unwrap_or(false),
                });
                let data = client
                    .web_agent(payload, Duration::from_secs(timeout_secs))
                    .await?;
                Ok(json!({ "status": "success", "action": "fill_form", "result": data }))
            }
            "select_option" => {
                let payload = json!({
                    "action": "select_option",
                    "index": parse_element_index(arguments),
                    "selector": extract_trimmed_non_empty_str(arguments, SELECTOR_ALIASES),
                    "value": arguments.get("value").or_else(|| arguments.get("option")).or_else(|| arguments.get("text")),
                });
                let data = client
                    .web_agent(payload, Duration::from_secs(timeout_secs))
                    .await?;
                Ok(json!({ "status": "success", "action": "select_option", "result": data }))
            }
            "hover" => {
                let payload = json!({
                    "action": "hover",
                    "index": parse_element_index(arguments),
                    "selector": extract_trimmed_non_empty_str(arguments, SELECTOR_ALIASES),
                    "label": arguments.get("label").and_then(|v| v.as_str()),
                });
                let data = client
                    .web_agent(payload, Duration::from_secs(timeout_secs))
                    .await?;
                Ok(json!({ "status": "success", "action": "hover", "result": data }))
            }
            "scroll" => {
                let payload = json!({
                    "action": "scroll",
                    "direction": arguments.get("direction").and_then(|v| v.as_str()).unwrap_or("down"),
                    "amount": arguments.get("amount").and_then(|v| v.as_i64()),
                    "scroll_id": arguments.get("scroll_id").and_then(|v| v.as_str()),
                    "selector": arguments.get("selector").and_then(|v| v.as_str()),
                });
                let data = client
                    .web_agent(payload, Duration::from_secs(timeout_secs))
                    .await?;
                Ok(json!({ "status": "success", "action": "scroll", "result": data }))
            }
            "press_key" => {
                let key = arguments
                    .get("key")
                    .and_then(|v| v.as_str())
                    .or_else(|| extract_trimmed_non_empty_str(arguments, TEXT_ALIASES))
                    .unwrap_or("Enter");
                let data = client
                    .web_agent(
                        json!({ "action": "press_key", "key": key }),
                        Duration::from_secs(timeout_secs),
                    )
                    .await?;
                Ok(json!({ "status": "success", "action": "press_key", "result": data }))
            }
            "wait_for" => {
                let payload = json!({
                    "action": "wait_for",
                    "selector": arguments.get("selector").and_then(|v| v.as_str()),
                    "text": arguments.get("text").and_then(|v| v.as_str()),
                    "gone": arguments.get("gone").and_then(|v| v.as_bool()).unwrap_or(false),
                    "timeout_ms": timeout_ms,
                });
                let data = client
                    .web_agent(payload, Duration::from_secs(timeout_secs + 3))
                    .await?;
                Ok(json!({ "status": "success", "action": "wait_for", "result": data }))
            }
            "eval_js" => {
                let script = extract_trimmed_non_empty_str(arguments, SCRIPT_ALIASES)
                    .ok_or_else(|| anyhow!("browser action='eval_js' requires a 'script' parameter"))?;
                let data = client
                    .eval_js(script, Duration::from_secs(timeout_secs))
                    .await?;
                Ok(json!({
                    "status": "success",
                    "action": "eval_js",
                    "url": data.get("url").cloned().unwrap_or(Value::Null),
                    "title": data.get("title").cloned().unwrap_or(Value::Null),
                    "output": data.get("output").cloned().unwrap_or(Value::Null),
                    "value": data.get("value").cloned().unwrap_or(Value::Null),
                }))
            }
            "screenshot" => {
                crate::tools::resource_policy::ensure_artifact_write_allowed("browser")?;
                let raw_out = extract_trimmed_non_empty_str(arguments, PATH_ALIASES)
                    .unwrap_or("browser_screenshot.png");
                let out_path = resolve_path(raw_out);
                let snap = client
                    .screenshot(&ScreenshotRequest {
                        path: out_path.clone(),
                        full_page: arguments
                            .get("full_page")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        transparent: arguments
                            .get("transparent")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        annotated: arguments
                            .get("annotated")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        selector: arguments
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string),
                    })
                    .await?;
                Ok(json!({
                    "status": "success",
                    "action": "screenshot",
                    "output_path": out_path.to_string_lossy(),
                    "width": snap.get("width").cloned().unwrap_or(Value::Null),
                    "height": snap.get("height").cloned().unwrap_or(Value::Null),
                }))
            }
            "save_pdf" => {
                crate::tools::resource_policy::ensure_artifact_write_allowed("browser")?;
                let raw_out = extract_trimmed_non_empty_str(arguments, PATH_ALIASES)
                    .unwrap_or("browser_page.pdf");
                let out_path = resolve_path(raw_out);
                let pdf = client.save_pdf(&out_path).await?;
                Ok(json!({
                    "status": "success",
                    "action": "save_pdf",
                    "output_path": out_path.to_string_lossy(),
                    "pages": pdf.get("pages").cloned().unwrap_or(Value::Null),
                }))
            }
            "back" | "forward" | "reload" => {
                let res = client
                    .request(
                        json!({
                            "op": "navigate",
                            "nav": action,
                            "timeout_ms": timeout_ms,
                            "settle_ms": settle_ms,
                        }),
                        Duration::from_secs(timeout_secs + 3),
                    )
                    .await?;
                Ok(json!({
                    "status": "success",
                    "action": action,
                    "url": res.get("url").cloned().unwrap_or(Value::Null),
                    "title": res.get("title").cloned().unwrap_or(Value::Null),
                }))
            }
            "resize" => {
                let scale = arguments
                    .get("scale")
                    .or_else(|| arguments.get("device_scale_factor"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                let res = client.resize(spec.width, spec.height, scale).await?;
                Ok(json!({
                    "status": "success",
                    "action": "resize",
                    "viewport": res.get("viewport").cloned().unwrap_or(Value::Null),
                }))
            }
            "mode" => {
                let res = client.set_mode(spec.gui).await?;
                Ok(json!({
                    "status": "success",
                    "action": "mode",
                    "mode": res.get("mode").cloned().unwrap_or(Value::Null),
                }))
            }
            "logs" | "network" => {
                let data = client
                    .web_agent(
                        json!({
                            "action": action,
                            "limit": arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(80),
                            "clear": arguments.get("clear").and_then(|v| v.as_bool()).unwrap_or(false),
                        }),
                        Duration::from_secs(5),
                    )
                    .await?;
                Ok(json!({
                    "status": "success",
                    "action": action,
                    "data": data,
                }))
            }
            "inspect_overlay" => {
                let enabled = arguments
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let data = client
                    .web_agent(
                        json!({ "action": "inspect_overlay", "enabled": enabled }),
                        Duration::from_secs(5),
                    )
                    .await?;
                Ok(json!({
                    "status": "success",
                    "action": "inspect_overlay",
                    "data": data,
                }))
            }
            "takeover_mask" => {
                let data = client
                    .web_agent(
                        json!({
                            "action": "takeover_mask",
                            "enabled": arguments.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                            "paused": arguments.get("paused").and_then(|v| v.as_bool()),
                            "status_text": arguments.get("text").and_then(|v| v.as_str()),
                        }),
                        Duration::from_secs(5),
                    )
                    .await?;
                Ok(json!({
                    "status": "success",
                    "action": "takeover_mask",
                    "data": data,
                }))
            }
            other => Err(anyhow!(
                "Unsupported browser action '{other}'. Supported actions: navigate, ui_tree, render, html, text, click, type, fill_form, select_option, hover, scroll, press_key, wait_for, eval_js, screenshot, save_pdf, back, forward, reload, resize, mode, logs, network, inspect_overlay, takeover_mask, status, close."
            )),
        }
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
