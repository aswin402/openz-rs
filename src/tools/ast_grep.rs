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

        let bin_path = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("ast-grep");
            if p.exists() { p } else { std::path::PathBuf::from("ast-grep") }
        } else {
            std::path::PathBuf::from("ast-grep")
        };

        let bin_path_for_spawn = bin_path;
        let pattern_for_spawn = pattern.to_string();
        let lang_for_spawn = lang.to_string();
        let path_arg = arguments.get("path")
            .or(arguments.get("TargetFile"))
            .or(arguments.get("filepath"))
            .or(arguments.get("file"))
            .or(arguments.get("Path"))
            .and_then(|v| v.as_str())
            .map(crate::config::resolve_path);

        let output = tokio::task::spawn_blocking(move || {
            let mut cmd = Command::new(&bin_path_for_spawn);
            crate::config::loader::set_command_cwd(&mut cmd);
            cmd.arg("run");
            cmd.arg("--pattern").arg(&pattern_for_spawn);
            cmd.arg("--lang").arg(&lang_for_spawn);
            cmd.arg("--json");
            if let Some(ref resolved) = path_arg {
                cmd.arg(resolved);
            }
            cmd.output()
        }).await??;
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

pub struct IndexCodebaseTool;

#[async_trait::async_trait]
impl Tool for IndexCodebaseTool {
    fn name(&self) -> &str {
        "index_codebase"
    }

    fn description(&self) -> &str {
        "Scan and index the workspace's structural code elements (functions, structs, classes, enums) using ast-grep and cache them into the local vector database for fast semantic lookups."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "lang": {
                    "type": "string",
                    "description": "The programming language to scan (e.g. 'rust', 'python', 'javascript', 'typescript', 'go')."
                },
                "path": {
                    "type": "string",
                    "description": "Optional directory or file path to restrict the scan to."
                }
            },
            "required": ["lang"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let lang = arguments.get("lang").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'lang' parameter"))?;

        let bin_path = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("ast-grep");
            if p.exists() { p } else { std::path::PathBuf::from("ast-grep") }
        } else {
            std::path::PathBuf::from("ast-grep")
        };

        let patterns = match lang.to_lowercase().as_str() {
            "rust" => vec![
                "fn $NAME($$$) { $$$ }",
                "struct $NAME { $$$ }",
                "enum $NAME { $$$ }",
                "impl $$$ { $$$ }"
            ],
            "python" => vec![
                "def $NAME($$$): $$$",
                "class $NAME($$$): $$$",
                "class $NAME: $$$"
            ],
            "javascript" | "typescript" => vec![
                "function $NAME($$$) { $$$ }",
                "class $NAME { $$$ }",
                "const $NAME = ($$$) => { $$$ }",
                "let $NAME = ($$$) => { $$$ }"
            ],
            "go" => vec![
                "func $NAME($$$) { $$$ }",
                "type $NAME struct { $$$ }",
                "type $NAME interface { $$$ }"
            ],
            _ => vec![
                "class $NAME { $$$ }",
                "function $NAME($$$) { $$$ }"
            ]
        };

        let mut indexed_count = 0;
        let mut entries_to_archive = Vec::new();

        for pattern in patterns {
            let mut cmd = Command::new(&bin_path);
            crate::config::loader::set_command_cwd(&mut cmd);
            cmd.arg("run");
            cmd.arg("--pattern").arg(pattern);
            cmd.arg("--lang").arg(lang);
            cmd.arg("--json");

            if let Some(path_str) = arguments.get("path")
                .or(arguments.get("TargetFile"))
                .or(arguments.get("filepath"))
                .or(arguments.get("file"))
                .or(arguments.get("Path"))
                .and_then(|v| v.as_str())
            {
                let resolved = crate::config::resolve_path(path_str);
                cmd.arg(resolved);
            }

            let output = cmd.output()?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();

            if output.status.success() && !stdout.trim().is_empty() {
                if let Ok(Value::Array(matches)) = serde_json::from_str::<Value>(&stdout) {
                    for item in matches {
                        let text = item["text"].as_str().unwrap_or_default().to_string();
                        let file = item["file"].as_str().unwrap_or_default();
                        let start_line = item["range"]["start"]["line"].as_u64().unwrap_or(0);
                        
                        let symbol_name = item["metaVariables"]["single"]["NAME"]["text"].as_str()
                            .unwrap_or("unknown_symbol");

                        let query_str = format!("Symbol: {} | Language: {} | File: {}", symbol_name, lang, file);
                        let source_str = format!("codebase_index: {}:{}", file, start_line + 1);

                        entries_to_archive.push((query_str, text, source_str));
                        indexed_count += 1;
                    }
                }
            }
        }

        if !entries_to_archive.is_empty() {
            let _ = crate::tools::shared_memory::archive_research_entries(entries_to_archive).await;
        }

        Ok(json!({
            "status": "success",
            "indexed_count": indexed_count,
            "message": format!("Successfully scanned and indexed {} structural elements from codebase.", indexed_count)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ast_grep_status() -> Result<()> {
        let tool = AstGrepTool;
        let args = json!({
            "pattern": "fn $X($$$)",
            "lang": "rust"
        });
        
        let res = tool.call(&args).await?;
        assert!(res.is_array());
        assert_eq!(tool.name(), "ast_grep");
        assert!(tool.description().contains("structural"));
        Ok(())
    }
}
