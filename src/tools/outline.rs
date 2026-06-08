use crate::tools::Tool;
use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

pub struct CodeOutlineTool;

#[derive(serde::Serialize)]
struct Symbol {
    line: usize,
    kind: String,
    name: String,
    definition: String,
}

#[async_trait::async_trait]
impl Tool for CodeOutlineTool {
    fn name(&self) -> &str {
        "code_outline"
    }

    fn description(&self) -> &str {
        "Extract definitions (classes, structs, functions, traits, methods) from a source file to understand its structure without reading the whole file."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the source file to parse (e.g. 'src/main.rs')."
                }
            },
            "required": ["file_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path_str = arguments.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'file_path' parameter"))?;
        let file_path = PathBuf::from(file_path_str);

        if !file_path.exists() {
            return Err(anyhow!("File '{}' does not exist", file_path_str));
        }

        let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        let content = fs::read_to_string(&file_path)?;
        let mut symbols = Vec::new();

        // Compile regexes for different languages
        let re_rust = Regex::new(r"^\s*(pub\s+)?(fn|struct|enum|trait|impl|type)\s+([a-zA-Z0-9_<>]+)")?;
        let re_python = Regex::new(r"^\s*(def|class)\s+([a-zA-Z0-9_]+)")?;
        let re_go = Regex::new(r"^\s*(func|type)\s+(\([^\)]+\)\s+)?([a-zA-Z0-9_]+)")?;
        let re_js_ts = Regex::new(r"^\s*(export\s+)?(function|class|interface|type|const|let)\s+([a-zA-Z0-9_]+)")?;

        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("#") || trimmed.starts_with("/*") || trimmed.is_empty() {
                continue;
            }

            match ext.as_str() {
                "rs" => {
                    if let Some(cap) = re_rust.captures(line) {
                        symbols.push(Symbol {
                            line: line_num,
                            kind: cap.get(2).unwrap().as_str().to_string(),
                            name: cap.get(3).unwrap().as_str().to_string(),
                            definition: trimmed.to_string(),
                        });
                    }
                }
                "py" => {
                    if let Some(cap) = re_python.captures(line) {
                        symbols.push(Symbol {
                            line: line_num,
                            kind: cap.get(1).unwrap().as_str().to_string(),
                            name: cap.get(2).unwrap().as_str().to_string(),
                            definition: trimmed.to_string(),
                        });
                    }
                }
                "go" => {
                    if let Some(cap) = re_go.captures(line) {
                        symbols.push(Symbol {
                            line: line_num,
                            kind: cap.get(1).unwrap().as_str().to_string(),
                            name: cap.get(3).unwrap().as_str().to_string(),
                            definition: trimmed.to_string(),
                        });
                    }
                }
                "js" | "ts" | "jsx" | "tsx" => {
                    if let Some(cap) = re_js_ts.captures(line) {
                        symbols.push(Symbol {
                            line: line_num,
                            kind: cap.get(2).unwrap().as_str().to_string(),
                            name: cap.get(3).unwrap().as_str().to_string(),
                            definition: trimmed.to_string(),
                        });
                    }
                }
                _ => {
                    // Fallback generic scanner
                    if trimmed.contains("fn ") || trimmed.contains("def ") || trimmed.contains("function ") || trimmed.contains("class ") || trimmed.contains("struct ") {
                        symbols.push(Symbol {
                            line: line_num,
                            kind: "unknown".to_string(),
                            name: trimmed.split_whitespace().nth(1).unwrap_or("").to_string(),
                            definition: trimmed.to_string(),
                        });
                    }
                }
            }
        }

        Ok(json!({
            "status": "success",
            "file": file_path_str,
            "symbols": symbols
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_code_outline() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("openz_outline_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        let rust_file = temp_dir.join("main.rs");
        std::fs::write(&rust_file, "
            pub fn run_app() {
                println!(\"Hello!\");
            }
            struct Config {
                port: u16,
            }
        ")?;

        let tool = CodeOutlineTool;
        let res = tool.call(&json!({
            "file_path": rust_file.to_str().unwrap()
        })).await?;

        assert_eq!(res["status"], "success");
        let symbols = res["symbols"].as_array().unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0]["kind"], "fn");
        assert_eq!(symbols[0]["name"], "run_app");
        assert_eq!(symbols[1]["kind"], "struct");
        assert_eq!(symbols[1]["name"], "Config");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
