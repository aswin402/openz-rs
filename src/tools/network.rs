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
                "port": { "type": "integer", "description": "The TCP port to inspect" },
                "host": { "type": "string", "description": "The host or IP to check (defaults to '127.0.0.1')" },
                "action": {
                    "type": "string",
                    "enum": ["check_free", "check_listening"],
                    "description": "Whether to check if the port is free to bind (check_free) or active and listening (check_listening)."
                }
            },
            "required": ["port", "action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let port = arguments.get("port").and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("Missing 'port' parameter"))?;
        let host = arguments.get("host").and_then(|v| v.as_str()).unwrap_or("127.0.0.1");
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let address = format!("{}:{}", host, port);

        match action {
            "check_free" => {
                match TcpListener::bind(&address) {
                    Ok(_) => Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "available": true,
                        "message": format!("Port {} is free and available to bind.", port)
                    })),
                    Err(_) => Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "available": false,
                        "message": format!("Port {} is occupied or unavailable to bind.", port)
                    })),
                }
            }
            "check_listening" => {
                let socket_addrs = address.to_socket_addrs()?
                    .next()
                    .ok_or_else(|| anyhow!("Invalid socket address"))?;
                
                match TcpStream::connect_timeout(&socket_addrs, Duration::from_millis(1000)) {
                    Ok(_) => Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "listening": true,
                        "message": format!("Port {} is active and listening.", port)
                    })),
                    Err(_) => Ok(json!({
                        "status": "success",
                        "port": port,
                        "host": host,
                        "listening": false,
                        "message": format!("Port {} is not listening or timed out.", port)
                    })),
                }
            }
            _ => Err(anyhow!("Invalid action: {}", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_port() -> Result<()> {
        let tool = CheckPortTool;
        let res = tool.call(&json!({
            "port": 58291,
            "action": "check_free"
        })).await?;
        assert_eq!(res["status"], "success");
        assert!(res["available"].as_bool().is_some());
        Ok(())
    }
}
