use crate::memory::{scope_from_args, with_graph_db};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tokio::process::Command;

pub struct AstGrepTool;

fn resolve_ast_grep_bin() -> std::path::PathBuf {
    dirs::home_dir()
        .map(|home| home.join(".cargo").join("bin").join("ast-grep"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("ast-grep"))
}

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
            .or_else(|| arguments.get("query"))
            .or_else(|| arguments.get("ast_pattern"))
            .or_else(|| arguments.get("rule"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Missing 'pattern' parameter"))?;
        let lang = arguments
            .get("lang")
            .or_else(|| arguments.get("language"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Missing 'lang' parameter"))?;

        let bin_path_for_spawn = resolve_ast_grep_bin();
        let pattern_for_spawn = pattern.to_string();
        let lang_for_spawn = lang.to_string();
        let path_arg = arguments
            .get("path")
            .or_else(|| arguments.get("dir"))
            .or_else(|| arguments.get("directory"))
            .or_else(|| arguments.get("file"))
            .or_else(|| arguments.get("file_path"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(crate::config::resolve_path);

        let mut cmd = Command::new(&bin_path_for_spawn);
        crate::config::loader::set_tokio_command_cwd(&mut cmd);
        cmd.arg("run");
        cmd.arg("--pattern").arg(&pattern_for_spawn);
        cmd.arg("--lang").arg(&lang_for_spawn);
        cmd.arg("--json");
        let _ = path_arg.as_ref().map(|resolved| cmd.arg(resolved));
        let output = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        match !output.status.success() && stdout.trim().is_empty() {
            true => Err(anyhow!("ast-grep execution failed: {}", stderr)),
            false => {
                let parsed_json: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
                    json!({
                        "raw_output": stdout,
                        "error": stderr
                    })
                });
                Ok(parsed_json)
            }
        }
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
    let is_fn = pattern.starts_with("fn ")
        || pattern.starts_with("def ")
        || pattern.starts_with("func ")
        || pattern.starts_with("function ")
        || pattern.contains("=>");
    let is_struct = pattern.starts_with("struct ") || pattern.contains(" struct ");
    let is_enum = pattern.starts_with("enum ");
    let is_impl = pattern.starts_with("impl ");
    let is_class = pattern.starts_with("class ");
    let is_interface = pattern.starts_with("type ") && pattern.contains(" interface ");
    let is_type_alias = pattern.starts_with("type ");

    match (
        is_fn,
        is_struct,
        is_enum,
        is_impl,
        is_class,
        is_interface,
        is_type_alias,
    ) {
        (true, _, _, _, _, _, _) => "Function",
        (false, true, _, _, _, _, _) => "Struct",
        (false, false, true, _, _, _, _) => "Enum",
        (false, false, false, true, _, _, _) => "ImplBlock",
        (false, false, false, false, true, _, _) => "Class",
        (false, false, false, false, false, true, _) => "Interface",
        (false, false, false, false, false, false, true) => "TypeAlias",
        (false, false, false, false, false, false, false) => "CodeElement",
    }
}

fn ast_grep_symbol_name(pattern: &str, item: &Value, text: &str) -> String {
    let meta_name = item["metaVariables"]["single"]["NAME"]["text"]
        .as_str()
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .map(str::to_string);
    match meta_name {
        Some(name) => name,
        None => {
            let first_line = text.lines().next().unwrap_or_default().trim();
            let kind = ast_grep_element_type(pattern);
            let impl_name = (kind == "ImplBlock")
                .then(|| first_line.strip_prefix("impl "))
                .flatten()
                .map(|rest| {
                    let target = rest
                        .split('{')
                        .next()
                        .unwrap_or(rest)
                        .split_whitespace()
                        .last()
                        .unwrap_or("unknown")
                        .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    format!("impl_{}", target)
                });
            impl_name.unwrap_or_else(|| {
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
            })
        }
    }
}

fn ast_grep_match_to_code_element(pattern: &str, item: &Value) -> Option<AstGrepCodeElement> {
    let text = item["text"]
        .as_str()
        .filter(|s| !s.trim().is_empty())?
        .to_string();
    let file_path = item["file"]
        .as_str()
        .filter(|s| !s.trim().is_empty())?
        .to_string();

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

        let bin_path = resolve_ast_grep_bin();

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

        let (user_id, session_id, agent_id) = scope_from_args(arguments);
        let mut indexed_count = 0;
        let mut code_graph_count = 0;
        let mut entries_to_archive = Vec::new();

        with_graph_db(|conn| {
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

            let _ = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .map(crate::config::resolve_path)
                .map(|resolved| cmd.arg(resolved));

            let output = cmd.output().await?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();

            let matches_opt = (output.status.success() && !stdout.trim().is_empty())
                .then(|| serde_json::from_str::<Value>(&stdout).ok())
                .flatten();
            let matches = match matches_opt {
                Some(Value::Array(items)) => items,
                _ => Vec::new(),
            };
            for item in matches {
                let text = item["text"].as_str().unwrap_or_default().to_string();
                let element = match ast_grep_match_to_code_element(pattern, &item) {
                    Some(elem) => elem,
                    None => continue,
                };
                let file = element.file_path.clone();
                let start_line = element.start_line;
                let symbol_name = element.name.clone();

                with_graph_db(|conn| {
                    insert_ast_grep_code_element(conn, &element, &user_id, &session_id, &agent_id)
                })?;
                code_graph_count += 1;

                let query_str = format!("Symbol: {symbol_name} | Language: {lang} | File: {file}");
                let source_str = format!("codebase_index: {}:{}", file, start_line);

                entries_to_archive.push((query_str, text, source_str));
                indexed_count += 1;
            }
        }

        match entries_to_archive.is_empty() {
            true => {}
            false => {
                let _ =
                    crate::tools::shared_memory::archive_research_entries(entries_to_archive).await;
            }
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
#[path = "ast_grep_tests.rs"]
mod tests;
