use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct OnpkgTool;

impl OnpkgTool {
    fn resolve_binary() -> String {
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("onpkg");
            if p.exists() {
                return p.to_string_lossy().to_string();
            }
        }
        "onpkg".to_string()
    }
}

#[async_trait::async_trait]
impl Tool for OnpkgTool {
    fn name(&self) -> &str {
        "onpkg"
    }

    fn description(&self) -> &str {
        "Use onpkg to list available templates/stacks, show details of a stack, scaffold a stack (website, app, backend, frontend from scratch), run environment diagnostics, or register/add new stacks, templates, skills, and packages for future use."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "list_stacks", 
                        "show_stack", 
                        "scaffold", 
                        "doctor", 
                        "add_template", 
                        "add_skill", 
                        "add_package", 
                        "install_package"
                    ],
                    "description": "The onpkg action to perform."
                },
                "stack_name": {
                    "type": "string",
                    "description": "The name of the template/stack to scaffold or show details for (required for 'scaffold' and 'show_stack')."
                },
                "dir": {
                    "type": "string",
                    "description": "The target directory to scaffold the stack into (optional for 'scaffold', defaults to current directory)."
                },
                "name": {
                    "type": "string",
                    "description": "The name for the template, skill, or package to add/install (required for 'add_template', 'add_skill', 'add_package', 'install_package')."
                },
                "source": {
                    "type": "string",
                    "description": "The path to the local directory/file or the remote git URL to add (required for 'add_template', 'add_skill')."
                },
                "runtime": {
                    "type": "string",
                    "enum": ["npm", "pypi", "pub", "cargo"],
                    "description": "The runtime category for packages (optional for package actions)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let onpkg_bin = Self::resolve_binary();
        let mut cmd = Command::new(&onpkg_bin);

        match action {
            "list_stacks" => {
                cmd.args(&["stack", "list"]);
            }
            "show_stack" => {
                let stack = arguments.get("stack_name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'stack_name' parameter for show_stack action"))?;
                cmd.args(&["stack", "show", stack]);
            }
            "scaffold" => {
                let stack = arguments.get("stack_name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'stack_name' parameter for scaffold action"))?;
                cmd.args(&["stack", "add", stack]);
                if let Some(dir) = arguments.get("dir").and_then(|v| v.as_str()) {
                    let resolved = crate::config::resolve_path(dir);
                    cmd.arg("--dir");
                    cmd.arg(resolved.to_string_lossy().to_string());
                }
            }
            "doctor" => {
                cmd.arg("doctor");
            }
            "add_template" => {
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for add_template action"))?;
                let source = arguments.get("source").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'source' parameter for add_template action"))?;
                cmd.args(&["template", "add", name, source]);
            }
            "add_skill" => {
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for add_skill action"))?;
                let source = arguments.get("source").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'source' parameter for add_skill action"))?;
                cmd.args(&["skill", "add", name, source]);
            }
            "add_package" => {
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for add_package action"))?;
                cmd.args(&["pkg", "add", name]);
                if let Some(rt) = arguments.get("runtime").and_then(|v| v.as_str()) {
                    cmd.arg("--runtime");
                    cmd.arg(rt);
                }
            }
            "install_package" => {
                let name = arguments.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'name' parameter for install_package action"))?;
                cmd.args(&["pkg", "install", name]);
                if let Some(rt) = arguments.get("runtime").and_then(|v| v.as_str()) {
                    cmd.arg("--runtime");
                    cmd.arg(rt);
                }
            }
            _ => return Err(anyhow!("Unsupported onpkg action: {}", action)),
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(json!({
            "status": if output.status.success() { "success" } else { "error" },
            "stdout": stdout,
            "stderr": stderr,
            "code": output.status.code()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_onpkg_tool() -> Result<()> {
        let tool = OnpkgTool;
        let res = tool.call(&json!({
            "action": "doctor"
        })).await?;

        assert_eq!(res["status"], "success");
        assert!(res["stdout"].as_str().unwrap().contains("Doctor complete"));

        Ok(())
    }
}
