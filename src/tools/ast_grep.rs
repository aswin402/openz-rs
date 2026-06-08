use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct AstGrepTool;

#[async_trait::async_trait]
impl Tool for AstGrepTool {
    fn name(&self) -> &str {
        "ast_grep"
    }

    fn description(&self) -> &str {
        "Perform a structural code search using AST patterns (e.g. 'fn $NAME($$$) { $$$ }') over the workspace."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The AST pattern to search for (e.g. '$A.map($B)' or 'struct $NAME { $$$ }')."
                },
                "lang": {
                    "type": "string",
                    "description": "The language target (e.g. 'rust', 'python', 'javascript', 'typescript', 'go')."
                },
                "path": {
                    "type": "string",
                    "description": "Optional subdirectory or file to restrict the search to."
                }
            },
            "required": ["pattern", "lang"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let pattern = arguments.get("pattern").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'pattern' parameter"))?;
        let lang = arguments.get("lang").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'lang' parameter"))?;

        let mut cmd = Command::new("ast-grep");
        cmd.arg("run");
        cmd.arg("--pattern").arg(pattern);
        cmd.arg("--lang").arg(lang);
        cmd.arg("--json");

        if let Some(path_str) = arguments.get("path").and_then(|v| v.as_str()) {
            let resolved = crate::config::resolve_path(path_str);
            cmd.arg(resolved);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && stdout.trim().is_empty() {
            return Err(anyhow!("ast-grep execution failed: {}", stderr));
        }

        let parsed_json: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
            json!({
                "raw_output": stdout,
                "error": stderr
            })
        });

        Ok(parsed_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ast_grep_status() -> Result<()> {
        let tool = AstGrepTool;
        let _args = json!({
            "pattern": "fn $X()",
            "lang": "rust"
        });
        
        assert_eq!(tool.name(), "ast_grep");
        assert!(tool.description().contains("structural"));
        Ok(())
    }
}
