use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

pub struct CheckPortTool;

#[async_trait::async_trait]
impl Tool for CheckPortTool {
    fn name(&self) -> &str {
        "check_port"
    }

    fn description(&self) -> &str {
        "Check if a specific TCP port is active (listening) or free on a host/IP address."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "port": {
                    "description": "The TCP port to inspect (integer or numeric string).",
                    "oneOf": [
                        { "type": "integer" },
                        { "type": "string" }
                    ]
                },
                "host": { "type": "string", "description": "The host or IP to check (defaults to '127.0.0.1')" },
                "action": {
                    "type": "string",
                    "enum": ["check_free", "check_listening"],
                    "description": "Whether to check if the port is free to bind (check_free) or active and listening (check_listening, default)."
                }
            },
            "required": ["port"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let port = arguments
            .get("port")
            .or_else(|| arguments.get("port_number"))
            .and_then(|v| {
                v.as_u64().or_else(|| {
                    v.as_str().and_then(|s| s.trim().parse::<u64>().ok())
                })
            })
            .ok_or_else(|| anyhow!("Missing 'port' parameter"))?;
        let host = arguments
            .get("host")
            .or_else(|| arguments.get("ip"))
            .or_else(|| arguments.get("target"))
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1");
        let action_raw = arguments
            .get("action")
            .or_else(|| arguments.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("check_listening");
        let action_norm = action_raw.trim().to_lowercase();
        let action = match action_norm.as_str() {
            "check_listening" | "listen" | "listening" | "active" | "open" | "status" => "check_listening",
            "check_free" | "free" | "available" | "bind" => "check_free",
            _ => action_norm.as_str(),
        };

        // Restrict to localhost to prevent internal network enumeration
        let allowed_hosts = ["127.0.0.1", "localhost", "::1", "[::1]"];
        let host_lower = host.to_lowercase();
        if !allowed_hosts.contains(&host_lower.as_str()) {
            // Also allow resolving to localhost
            let host_clone = host.to_string();
            let resolves_to_localhost = tokio::task::spawn_blocking(move || {
                use std::net::ToSocketAddrs;
                format!("{}:0", host_clone)
                    .to_socket_addrs()
                    .map(|mut iter| {
                        iter.any(|addr| addr.ip().is_loopback())
                    })
                    .unwrap_or(false)
            })
            .await?;
            if !resolves_to_localhost {
                return Err(anyhow!(
                    "Security: check_port only allows localhost targets to prevent internal network enumeration. Got host: {}",
                    host
                ));
            }
        }

        let address = format!("{}:{}", host, port);

        match action {
            "check_free" => {
                let address_clone = address.clone();
                let available =
                    tokio::task::spawn_blocking(move || TcpListener::bind(&address_clone).is_ok())
                        .await?;

                if available {
                    Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "available": true,
                        "message": format!("Port {} is free and available to bind.", port)
                    }))
                } else {
                    Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "available": false,
                        "message": format!("Port {} is occupied or unavailable to bind.", port)
                    }))
                }
            }
            "check_listening" => {
                let address_clone = address.clone();
                let listening = tokio::task::spawn_blocking(move || {
                    if let Ok(mut addrs) = address_clone.to_socket_addrs() {
                        if let Some(socket_addr) = addrs.next() {
                            return TcpStream::connect_timeout(
                                &socket_addr,
                                Duration::from_millis(1000),
                            )
                            .is_ok();
                        }
                    }
                    false
                })
                .await?;

                if listening {
                    Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "listening": true,
                        "message": format!("Port {} is active and listening.", port)
                    }))
                } else {
                    Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "listening": false,
                        "message": format!("Port {} is not listening or timed out.", port)
                    }))
                }
            }
            _ => Err(anyhow!("Invalid action: {}", action)),
        }
    }
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
