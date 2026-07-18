use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tokio::process::Command;

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
        let pattern = arguments
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'pattern' parameter"))?;
        let lang = arguments
            .get("lang")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'lang' parameter"))?;

        let bin_path = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("ast-grep");
            if p.exists() {
                p
            } else {
                std::path::PathBuf::from("ast-grep")
            }
        } else {
            std::path::PathBuf::from("ast-grep")
        };

        let bin_path_for_spawn = bin_path;
        let pattern_for_spawn = pattern.to_string();
        let lang_for_spawn = lang.to_string();
        let path_arg = arguments
            .get("path")
            .or(arguments.get("TargetFile"))
            .or(arguments.get("filepath"))
            .or(arguments.get("file"))
            .or(arguments.get("Path"))
            .and_then(|v| v.as_str())
            .map(crate::config::resolve_path);

        let mut cmd = Command::new(&bin_path_for_spawn);
        crate::config::loader::set_tokio_command_cwd(&mut cmd);
        cmd.arg("run");
        cmd.arg("--pattern").arg(&pattern_for_spawn);
        cmd.arg("--lang").arg(&lang_for_spawn);
        cmd.arg("--json");
        if let Some(ref resolved) = path_arg {
            cmd.arg(resolved);
        }
        let output = cmd.output().await?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct AstGrepCodeElement {
    element_type: String,
    name: String,
    file_path: String,
    signature: String,
    start_line: i64,
    end_line: i64,
}

fn ast_grep_element_type(pattern: &str) -> &'static str {
    let pattern = pattern.trim_start();
    if pattern.starts_with("fn ")
        || pattern.starts_with("def ")
        || pattern.starts_with("func ")
        || pattern.starts_with("function ")
        || pattern.contains("=>")
    {
        "Function"
    } else if pattern.starts_with("struct ") || pattern.contains(" struct ") {
        "Struct"
    } else if pattern.starts_with("enum ") {
        "Enum"
    } else if pattern.starts_with("impl ") {
        "ImplBlock"
    } else if pattern.starts_with("class ") {
        "Class"
    } else if pattern.starts_with("type ") && pattern.contains(" interface ") {
        "Interface"
    } else if pattern.starts_with("type ") {
        "TypeAlias"
    } else {
        "CodeElement"
    }
}

fn ast_grep_symbol_name(pattern: &str, item: &Value, text: &str) -> String {
    if let Some(name) = item["metaVariables"]["single"]["NAME"]["text"].as_str() {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let first_line = text.lines().next().unwrap_or_default().trim();
    let kind = ast_grep_element_type(pattern);
    if kind == "ImplBlock" {
        if let Some(rest) = first_line.strip_prefix("impl ") {
            let target = rest
                .split('{')
                .next()
                .unwrap_or(rest)
                .split_whitespace()
                .last()
                .unwrap_or("unknown")
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            return format!("impl_{}", target);
        }
    }

    first_line
        .split_whitespace()
        .find(|part| {
            part.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        })
        .unwrap_or("unknown_symbol")
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
        .to_string()
}

fn ast_grep_match_to_code_element(pattern: &str, item: &Value) -> Option<AstGrepCodeElement> {
    let text = item["text"].as_str()?.to_string();
    let file_path = item["file"].as_str()?.to_string();
    if text.trim().is_empty() || file_path.trim().is_empty() {
        return None;
    }

    let start_line = item["range"]["start"]["line"].as_i64().unwrap_or(0) + 1;
    let end_line = item["range"]["end"]["line"]
        .as_i64()
        .map(|line| line + 1)
        .unwrap_or(start_line)
        .max(start_line);
    let signature = text
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(240)
        .collect::<String>();
    let name = ast_grep_symbol_name(pattern, item, &text);

    Some(AstGrepCodeElement {
        element_type: ast_grep_element_type(pattern).to_string(),
        name,
        file_path,
        signature,
        start_line,
        end_line,
    })
}

fn insert_ast_grep_code_element(
    conn: &Connection,
    element: &AstGrepCodeElement,
    user_id: &str,
    session_id: &str,
    agent_id: &str,
) -> Result<()> {
    let element_id = format!(
        "{}:{}:{}:{}",
        element.file_path, element.element_type, element.name, element.start_line
    );
    conn.execute(
        "INSERT OR REPLACE INTO code_elements (element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, ?8, ?9, ?10)",
        params![
            element_id,
            element.file_path,
            element.element_type,
            element.name,
            element.signature,
            element.start_line,
            element.end_line,
            user_id,
            session_id,
            agent_id,
        ],
    )?;
    Ok(())
}

pub struct AstGrepIndexCodebaseTool;

#[async_trait::async_trait]
impl Tool for AstGrepIndexCodebaseTool {
    fn name(&self) -> &str {
        "ast_grep_index_codebase"
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
        let lang = arguments
            .get("lang")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'lang' parameter"))?;

        let bin_path = if let Some(home) = dirs::home_dir() {
            let p = home.join(".cargo").join("bin").join("ast-grep");
            if p.exists() {
                p
            } else {
                std::path::PathBuf::from("ast-grep")
            }
        } else {
            std::path::PathBuf::from("ast-grep")
        };

        let patterns = match lang.to_lowercase().as_str() {
            "rust" => vec![
                "fn $NAME($$$) { $$$ }",
                "struct $NAME { $$$ }",
                "enum $NAME { $$$ }",
                "impl $$$ { $$$ }",
            ],
            "python" => vec![
                "def $NAME($$$): $$$",
                "class $NAME($$$): $$$",
                "class $NAME: $$$",
            ],
            "javascript" | "typescript" => vec![
                "function $NAME($$$) { $$$ }",
                "class $NAME { $$$ }",
                "const $NAME = ($$$) => { $$$ }",
                "let $NAME = ($$$) => { $$$ }",
            ],
            "go" => vec![
                "func $NAME($$$) { $$$ }",
                "type $NAME struct { $$$ }",
                "type $NAME interface { $$$ }",
            ],
            _ => vec!["class $NAME { $$$ }", "function $NAME($$$) { $$$ }"],
        };

        let (user_id, session_id, agent_id) =
            crate::tools::graph_memory::scope_from_args(arguments);
        let mut indexed_count = 0;
        let mut code_graph_count = 0;
        let mut entries_to_archive = Vec::new();

        crate::tools::graph_memory::with_db(|conn| {
            conn.execute(
                "DELETE FROM code_calls WHERE caller_id IN (SELECT element_id FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*'))",
                params![user_id, session_id, agent_id],
            )?;
            conn.execute(
                "DELETE FROM code_elements WHERE (user_id = ?1 OR user_id = '*') AND (session_id = ?2 OR session_id = '*') AND (agent_id = ?3 OR agent_id = '*')",
                params![user_id, session_id, agent_id],
            )?;
            Ok(())
        })?;

        for pattern in patterns {
            let mut cmd = Command::new(&bin_path);
            crate::config::loader::set_tokio_command_cwd(&mut cmd);
            cmd.arg("run");
            cmd.arg("--pattern").arg(pattern);
            cmd.arg("--lang").arg(lang);
            cmd.arg("--json");

            if let Some(path_str) = arguments
                .get("path")
                .or(arguments.get("TargetFile"))
                .or(arguments.get("filepath"))
                .or(arguments.get("file"))
                .or(arguments.get("Path"))
                .and_then(|v| v.as_str())
            {
                let resolved = crate::config::resolve_path(path_str);
                cmd.arg(resolved);
            }

            let output = cmd.output().await?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();

            if output.status.success() && !stdout.trim().is_empty() {
                if let Ok(Value::Array(matches)) = serde_json::from_str::<Value>(&stdout) {
                    for item in matches {
                        let text = item["text"].as_str().unwrap_or_default().to_string();
                        let Some(element) = ast_grep_match_to_code_element(pattern, &item) else {
                            continue;
                        };
                        let file = element.file_path.clone();
                        let start_line = element.start_line;
                        let symbol_name = element.name.clone();

                        crate::tools::graph_memory::with_db(|conn| {
                            insert_ast_grep_code_element(
                                conn,
                                &element,
                                &user_id,
                                &session_id,
                                &agent_id,
                            )
                        })?;
                        code_graph_count += 1;

                        let query_str =
                            format!("Symbol: {symbol_name} | Language: {lang} | File: {file}");
                        let source_str = format!("codebase_index: {}:{}", file, start_line);

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
            "codeGraphCount": code_graph_count,
            "message": format!("Successfully scanned and indexed {} structural elements from codebase.", indexed_count)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_grep_match_to_code_element_parses_symbol() {
        let item = json!({
            "text": "fn bridge_symbol(input: String) -> String { input }",
            "file": "src/example.rs",
            "range": { "start": { "line": 9 }, "end": { "line": 11 } },
            "metaVariables": { "single": { "NAME": { "text": "bridge_symbol" } } }
        });

        let element = ast_grep_match_to_code_element("fn $NAME($$$) { $$$ }", &item).unwrap();
        assert_eq!(element.element_type, "Function");
        assert_eq!(element.name, "bridge_symbol");
        assert_eq!(element.file_path, "src/example.rs");
        assert_eq!(element.start_line, 10);
        assert_eq!(element.end_line, 12);
        assert!(element.signature.contains("bridge_symbol"));
    }

    #[tokio::test]
    async fn test_ast_grep_inserted_elements_are_queryable_by_code_graph() -> Result<()> {
        let _lock = crate::tools::graph_memory::test_lock().lock().await;
        let session_id = format!("ast_grep_bridge_{}", uuid::Uuid::new_v4());
        let item = json!({
            "text": "struct BridgeQueryable { value: String }",
            "file": "src/bridge_queryable.rs",
            "range": { "start": { "line": 2 }, "end": { "line": 4 } },
            "metaVariables": { "single": { "NAME": { "text": "BridgeQueryable" } } }
        });
        let element = ast_grep_match_to_code_element("struct $NAME { $$$ }", &item).unwrap();
        crate::tools::graph_memory::with_db(|conn| {
            insert_ast_grep_code_element(conn, &element, "*", &session_id, "*")
        })?;

        let tool = crate::tools::memory_extra::QueryCodeGraphTool;
        let results = tool
            .call(&json!({"query": "BridgeQueryable", "session_id": session_id}))
            .await?;
        let items = results
            .as_array()
            .expect("query_code_graph returns an array");
        assert!(items.iter().any(|item| item["name"] == "BridgeQueryable"));
        Ok(())
    }

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
