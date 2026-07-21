# Native Browser Status Inspection Tool Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a native `inspect_browsers` tool for OpenZ to monitor running browser sessions, detect daemon connectivity issues, list open pages/tabs, and query recent browser execution errors.

**Architecture:** 
- A new native tool struct `InspectBrowsersTool` will query ports 4444 (GeckoDriver) and 9222 (Chrome CDP) to check for active connections.
- It will execute CLI health status checks against `gsd-browser` daemon processes.
- It will query the SQLite database (`logs.db`) using `rusqlite` to aggregate the latest warning/error browser events.

**Tech Stack:** Rust, reqwest, rusqlite, serde_json, tokio

---

### Task 1: Create the Browser Status Tool Module

**Files:**
- Create: `src/tools/browser_status.rs`

**Step 1: Write the tool structure and basic logic**

Write `src/tools/browser_status.rs` with the following implementation:

```rust
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::process::Command;
use std::time::Duration;

pub struct InspectBrowsersTool;

fn get_recent_browser_errors() -> Result<Vec<Value>> {
    let db_path = crate::config::config_dir().join("logs.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let conn = rusqlite::Connection::open(&db_path)?;
    let mut stmt = conn.prepare(
        \"SELECT timestamp, level, target, message 
         FROM logs 
         WHERE (message LIKE '%firefox%' OR message LIKE '%gsd-browser%' OR message LIKE '%obscura%' OR message LIKE '%browser%') 
           AND (level = 'ERROR' OR level = 'WARN') 
         ORDER BY timestamp DESC 
         LIMIT 10\"
    )?;
    
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            \"timestamp\": row.get::<_, String>(0)?,
            \"level\": row.get::<_, String>(1)?,
            \"target\": row.get::<_, String>(2)?,
            \"message\": row.get::<_, String>(3)?,
        }))
    })?;

    let mut errors = Vec::new();
    for row in rows {
        if let Ok(entry) = row {
            errors.push(entry);
        }
    }
    Ok(errors)
}

#[async_trait::async_trait]
impl Tool for InspectBrowsersTool {
    fn name(&self) -> &str {
        \"inspect_browsers\"
    }

    fn description(&self) -> &str {
        \"Inspect active browser status, running background daemons, open pages/tabs, displaying URLs, and recent browser errors.\"
    }

    fn parameters(&self) -> Value {
        json!({
            \"type\": \"object\",
            \"properties\": {}
        })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(800))
            .build()?;

        // 1. Check Firefox (GeckoDriver)
        let firefox_status = match client.get(\"http://127.0.0.1:4444/status\").send().await {
            Ok(res) => {
                if let Ok(val) = res.json::<Value>().await {
                    json!({
                        \"status\": \"running\",
                        \"details\": val
                    })
                } else {
                    json!({ \"status\": \"running\", \"message\": \"Failed to parse geckodriver status JSON\" })
                }
            }
            Err(_) => {
                // Check if port is open
                if tokio::net::TcpStream::connect(\"127.0.0.1:4444\").await.is_ok() {
                    json!({ \"status\": \"running\", \"message\": \"Port 4444 open, geckodriver status endpoint unresponsive\" })
                } else {
                    json!({ \"status\": \"stopped\" })
                }
            }
        };

        // 2. Check Chrome (Obscura CDP)
        let obscura_status = match client.get(\"http://127.0.0.1:9222/json/list\").send().await {
            Ok(res) => {
                if let Ok(val) = res.json::<Value>().await {
                    json!({
                        \"status\": \"running\",
                        \"pages\": val
                    })
                } else {
                    json!({ \"status\": \"running\", \"message\": \"Failed to parse Chrome JSON target list\" })
                }
            }
            Err(_) => {
                if tokio::net::TcpStream::connect(\"127.0.0.1:9222\").await.is_ok() {
                    json!({ \"status\": \"running\", \"message\": \"Port 9222 open, Chrome list endpoint unresponsive\" })
                } else {
                    json!({ \"status\": \"stopped\" })
                }
            }
        };

        // 3. Check gsd_browser
        let bin_path = if let Some(home) = dirs::home_dir() {
            let p = home.join(\".cargo\").join(\"bin\").join(\"gsd-browser\");
            if p.exists() {
                p
            } else {
                std::path::PathBuf::from(\"gsd-browser\")
            }
        } else {
            std::path::PathBuf::from(\"gsd-browser\")
        };

        let mut gsd_health_cmd = Command::new(&bin_path);
        gsd_health_cmd.arg(\"daemon\").arg(\"health\");
        let gsd_status = match gsd_health_cmd.output().await {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    // Try to list active pages
                    let mut gsd_pages_cmd = Command::new(&bin_path);
                    gsd_pages_cmd.arg(\"list-pages\").arg(\"--json\");
                    let pages_val = match gsd_pages_cmd.output().await {
                        Ok(p_out) if p_out.status.success() => {
                            let p_stdout = String::from_utf8_lossy(&p_out.stdout).to_string();
                            serde_json::from_str::<Value>(&p_stdout).unwrap_or_else(|_| json!(p_stdout.trim()))
                        }
                        _ => json!([])
                    };

                    json!({
                        \"status\": \"running\",
                        \"health\": stdout.trim(),
                        \"pages\": pages_val
                    })
                } else {
                    json!({
                        \"status\": \"stopped\",
                        \"error\": if stderr.trim().is_empty() { stdout.trim().to_string() } else { stderr.trim().to_string() }
                    })
                }
            }
            Err(e) => {
                json!({
                    \"status\": \"stopped\",
                    \"error\": format!(\"Failed to run gsd-browser binary: {:?}\", e)
                })
            }
        };

        // 4. Query recent logs
        let recent_errors = get_recent_browser_errors().unwrap_or_else(|_| vec![]);

        Ok(json!({
            \"firefox_geckodriver\": firefox_status,
            \"chrome_obscura\": obscura_status,
            \"gsd_browser\": gsd_status,
            \"recent_browser_errors\": recent_errors
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inspect_browsers_metadata() -> Result<()> {
        let tool = InspectBrowsersTool;
        assert_eq!(tool.name(), \"inspect_browsers\");
        Ok(())
    }
}
```

---

### Task 2: Register the Tool in tools module

**Files:**
- Modify: `src/tools/mod.rs`

**Step 1: Declare the module**

Add:
```rust
pub mod browser_status;
```
to `src/tools/mod.rs` list of pub modules.

**Step 2: Commit**

```bash
git add src/tools/browser_status.rs src/tools/mod.rs
git commit -m \"feat: add inspect_browsers tool module and declare it\"
```

---

### Task 3: Register the Tool in CLI Registry

**Files:**
- Modify: `src/cli/tools.rs`

**Step 1: Import the tool struct**

Import:
```rust
use crate::tools::browser_status::InspectBrowsersTool;
```

**Step 2: Register in `register_core_tools`**

Add registration:
```rust
registry.register(std::sync::Arc::new(InspectBrowsersTool));
```
inside `register_core_tools` function.

**Step 3: Run compiler checks and tests**

Run: `cargo check`
Run: `cargo test browser_status`

**Step 4: Commit**

```bash
git add src/cli/tools.rs
git commit -m \"feat: register inspect_browsers tool in CLI registry\"
```
