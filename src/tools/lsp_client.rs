//! Native JSON-RPC 2.0 Language Server Protocol (`lsp_query`) bridge tool.
//!
//! Provides on-demand semantic code navigation (`hover`, `definition`,
//! `references`, `document_symbols`, `workspace_symbols`) and local server
//! discovery (`servers`) across Rust (`rust-analyzer`), TypeScript/JavaScript
//! (`typescript-language-server` / `vtsls`), Python (`pyright-langserver` / `pylsp`),
//! Go (`gopls`), C/C++ (`clangd`), and Zig (`zls`).

use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub struct LspQueryTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Cpp,
    Zig,
}

impl LspLanguage {
    pub fn language_id(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Cpp => "cpp",
            Self::Zig => "zig",
        }
    }

    pub fn from_hint(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "rs" | "rust" => Some(Self::Rust),
            "ts" | "tsx" | "mts" | "cts" | "typescript" | "typescriptreact" => {
                Some(Self::TypeScript)
            }
            "js" | "jsx" | "mjs" | "cjs" | "javascript" | "javascriptreact" => {
                Some(Self::JavaScript)
            }
            "py" | "pyi" | "python" => Some(Self::Python),
            "go" | "golang" => Some(Self::Go),
            "c" | "cc" | "cpp" | "cxx" | "h" | "hpp" | "c++" => Some(Self::Cpp),
            "zig" => Some(Self::Zig),
            _ => None,
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_hint)
    }

    pub fn candidate_servers(self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            Self::Rust => &[("rust-analyzer", &[])],
            Self::TypeScript | Self::JavaScript => &[
                ("typescript-language-server", &["--stdio"]),
                ("vtsls", &["--stdio"]),
            ],
            Self::Python => &[("pyright-langserver", &["--stdio"]), ("pylsp", &[])],
            Self::Go => &[("gopls", &["serve"])],
            Self::Cpp => &[("clangd", &["--background-index=false"])],
            Self::Zig => &[("zls", &[])],
        }
    }
}

fn search_executable(bin_name: &str, workspace_root: &Path) -> Option<PathBuf> {
    let mut candidate_dirs: Vec<PathBuf> = vec![
        workspace_root.join("node_modules").join(".bin"),
        workspace_root.join("web").join("node_modules").join(".bin"),
    ];

    match dirs::home_dir() {
        Some(home) => {
            candidate_dirs.push(home.join(".cargo").join("bin"));
            candidate_dirs.push(home.join(".local").join("bin"));
        }
        None => {}
    }

    match std::env::var_os("PATH") {
        Some(path_env) => {
            candidate_dirs.extend(std::env::split_paths(&path_env));
        }
        None => {}
    }

    candidate_dirs
        .into_iter()
        .map(|dir| dir.join(bin_name))
        .find(|full| full.is_file())
}

pub fn detect_installed_lsp_servers(workspace_root: &Path) -> Vec<Value> {
    let all_langs = [
        LspLanguage::Rust,
        LspLanguage::TypeScript,
        LspLanguage::Python,
        LspLanguage::Go,
        LspLanguage::Cpp,
        LspLanguage::Zig,
    ];

    all_langs
        .into_iter()
        .map(|lang| {
            let resolved = lang
                .candidate_servers()
                .iter()
                .find_map(|(bin, args)| {
                    search_executable(bin, workspace_root).map(|path| (bin, args, path))
                });
            match resolved {
                Some((bin, args, path)) => json!({
                    "language": lang.language_id(),
                    "available": true,
                    "server": bin,
                    "args": args,
                    "binary_path": path.to_string_lossy(),
                }),
                None => json!({
                    "language": lang.language_id(),
                    "available": false,
                    "candidates": lang.candidate_servers().iter().map(|(b, _)| *b).collect::<Vec<_>>(),
                }),
            }
        })
        .collect()
}

pub fn path_to_file_uri(path: &Path) -> Result<String> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    url::Url::from_file_path(&canonical)
        .map(|u| u.to_string())
        .map_err(|_| anyhow!("Failed to convert path to file URI: {:?}", canonical))
}

pub fn file_uri_to_display_path(uri_str: &str, workspace_root: &Path) -> String {
    url::Url::parse(uri_str)
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .map(|full| {
            full.strip_prefix(workspace_root)
                .map(|rel| rel.to_string_lossy().to_string())
                .unwrap_or_else(|_| full.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| uri_str.to_string())
}

pub fn encode_lsp_frame(payload: &Value) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(payload)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut out = Vec::with_capacity(header.len() + body.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub async fn write_lsp_frame<W: AsyncWrite + Unpin>(writer: &mut W, payload: &Value) -> Result<()> {
    let frame = encode_lsp_frame(payload)?;
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_lsp_frame<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Result<Value> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        match n == 0 {
            true => return Err(anyhow!("LSP server closed stdout before sending header")),
            false => {}
        }
        let trimmed = line.trim();
        match trimmed.is_empty() {
            true => break,
            false => {
                let lower = trimmed.to_ascii_lowercase();
                match lower.strip_prefix("content-length:") {
                    Some(rest) => {
                        content_length = rest.trim().parse::<usize>().ok();
                    }
                    None => {}
                }
            }
        }
    }

    let len = content_length.ok_or_else(|| anyhow!("Missing Content-Length in LSP frame"))?;
    (len <= 32 * 1024 * 1024)
        .then_some(())
        .ok_or_else(|| anyhow!("LSP frame exceeds 32MB safety cap ({len} bytes)"))?;

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    let parsed: Value = serde_json::from_slice(&body)?;
    Ok(parsed)
}

async fn await_lsp_response<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut BufReader<R>,
    writer: &mut W,
    expected_id: u64,
) -> Result<Value> {
    for _ in 0..512 {
        let msg = read_lsp_frame(reader).await?;
        let has_method = msg.get("method").and_then(Value::as_str).is_some();
        let msg_id = msg.get("id").and_then(Value::as_u64);

        match (has_method, msg_id) {
            // Server-to-client request (e.g. client/registerCapability or workspace/configuration):
            // acknowledge with null result so the server never blocks.
            (true, Some(req_id)) => {
                let ack = json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": Value::Null,
                });
                let _ = write_lsp_frame(writer, &ack).await;
            }
            // Server notification (diagnostics, progress, logMessage): skip.
            (true, None) => {}
            // Response matching our request ID!
            (false, Some(resp_id)) => match resp_id == expected_id {
                true => match msg.get("error") {
                    Some(err_obj) => {
                        return Err(anyhow!("LSP server returned error: {}", err_obj));
                    }
                    None => {
                        return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
                    }
                },
                false => {}
            },
            (false, None) => {}
        }
    }
    Err(anyhow!(
        "Exceeded 512 LSP messages without receiving response for request #{expected_id}"
    ))
}

fn find_workspace_root(start: &Path) -> PathBuf {
    let mut cur = match start.is_file() {
        true => start
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| start.to_path_buf()),
        false => start.to_path_buf(),
    };

    let markers = [
        "Cargo.toml",
        "tsconfig.json",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "build.zig",
        ".git",
    ];

    loop {
        let found = markers.iter().any(|m| cur.join(m).exists());
        match found {
            true => return cur,
            false => match cur.pop() {
                true => {}
                false => {
                    return start
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from("."));
                }
            },
        }
    }
}

pub fn normalize_lsp_locations(raw: &Value, workspace_root: &Path) -> Vec<Value> {
    let items: Vec<&Value> = match raw {
        Value::Array(arr) => arr.iter().take(100).collect(),
        Value::Object(_) => vec![raw],
        _ => Vec::new(),
    };

    items
        .into_iter()
        .filter_map(|item| {
            let uri = item
                .get("uri")
                .or_else(|| item.get("targetUri"))
                .and_then(Value::as_str)?;
            let range = item
                .get("range")
                .or_else(|| item.get("targetSelectionRange"))
                .or_else(|| item.get("targetRange"))?;
            let start_line = range["start"]["line"].as_u64().unwrap_or(0) + 1;
            let start_char = range["start"]["character"].as_u64().unwrap_or(0) + 1;
            let end_line = range["end"]["line"].as_u64().unwrap_or(0) + 1;
            let end_char = range["end"]["character"].as_u64().unwrap_or(0) + 1;
            Some(json!({
                "file": file_uri_to_display_path(uri, workspace_root),
                "line": start_line,
                "character": start_char,
                "end_line": end_line,
                "end_character": end_char,
            }))
        })
        .collect()
}

pub fn extract_hover_markdown(raw: &Value) -> String {
    let contents = match raw.get("contents") {
        Some(c) => c,
        None => return String::new(),
    };
    match contents {
        Value::String(s) => s.trim().to_string(),
        Value::Object(map) => map
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|entry| match entry {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Object(m) => m
                    .get("value")
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn lsp_symbol_kind_name(kind: u64) -> &'static str {
    match kind {
        1 => "File",
        2 => "Module",
        3 => "Namespace",
        4 => "Package",
        5 => "Class",
        6 => "Method",
        7 => "Property",
        8 => "Field",
        9 => "Constructor",
        10 => "Enum",
        11 => "Interface",
        12 => "Function",
        13 => "Variable",
        14 => "Constant",
        15 => "String",
        22 => "EnumMember",
        23 => "Struct",
        26 => "TypeParameter",
        _ => "Symbol",
    }
}

fn flatten_document_symbols(items: &[Value], out: &mut Vec<Value>, workspace_root: &Path) {
    for item in items {
        let name = match item.get("name").and_then(Value::as_str) {
            Some(n) => n,
            None => continue,
        };
        let kind_num = item.get("kind").and_then(Value::as_u64).unwrap_or(0);
        let detail = item.get("detail").and_then(Value::as_str).unwrap_or("");

        let range = item
            .get("selectionRange")
            .or_else(|| item.get("range"))
            .or_else(|| item.get("location").and_then(|loc| loc.get("range")));
        let line = range
            .and_then(|r| r["start"]["line"].as_u64())
            .map(|l| l + 1)
            .unwrap_or(1);
        let end_line = item
            .get("range")
            .or_else(|| item.get("location").and_then(|loc| loc.get("range")))
            .and_then(|r| r["end"]["line"].as_u64())
            .map(|l| l + 1)
            .unwrap_or(line);

        let file_opt = item
            .get("location")
            .and_then(|loc| loc.get("uri"))
            .and_then(Value::as_str)
            .map(|u| file_uri_to_display_path(u, workspace_root));

        match file_opt {
            Some(file_str) => out.push(json!({
                "name": name,
                "kind": lsp_symbol_kind_name(kind_num),
                "file": file_str,
                "line": line,
                "end_line": end_line,
            })),
            None => out.push(json!({
                "name": name,
                "kind": lsp_symbol_kind_name(kind_num),
                "detail": detail,
                "line": line,
                "end_line": end_line,
            })),
        }

        match item.get("children").and_then(Value::as_array) {
            Some(children) => flatten_document_symbols(children, out, workspace_root),
            None => {}
        }
    }
}

#[async_trait::async_trait]
impl Tool for LspQueryTool {
    fn name(&self) -> &str {
        "lsp_query"
    }

    fn description(&self) -> &str {
        "Query a local Language Server Protocol (LSP) server on demand for type hover signatures, go-to-definition, find-references, document symbols, workspace symbols, or installed server discovery."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "servers",
                        "hover",
                        "definition",
                        "references",
                        "document_symbols",
                        "workspace_symbols"
                    ],
                    "description": "The LSP query operation to perform."
                },
                "file_path": {
                    "type": "string",
                    "description": "Source file path (required for hover, definition, references, document_symbols; optional hint for workspace_symbols)."
                },
                "line": {
                    "type": "integer",
                    "description": "1-indexed line number in file_path (required for hover, definition, references)."
                },
                "character": {
                    "type": "integer",
                    "description": "1-indexed column/character offset on the line (defaults to 1)."
                },
                "query": {
                    "type": "string",
                    "description": "Symbol query string for action='workspace_symbols'."
                },
                "language": {
                    "type": "string",
                    "description": "Optional explicit language override ('rust', 'typescript', 'javascript', 'python', 'go', 'cpp', 'zig')."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Optional timeout in seconds for the LSP query (5..=90, default 25)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?
            .to_ascii_lowercase();

        let default_cwd = crate::config::loader::resolve_path(".");

        match action.as_str() {
            "servers" | "status" | "detect" => {
                let servers = detect_installed_lsp_servers(&default_cwd);
                return Ok(json!({
                    "status": "success",
                    "servers": servers,
                }));
            }
            _ => {}
        }

        let file_path_opt = arguments
            .get("file_path")
            .or_else(|| arguments.get("path"))
            .or_else(|| arguments.get("file"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(crate::config::resolve_path);

        let lang = arguments
            .get("language")
            .or_else(|| arguments.get("lang"))
            .and_then(Value::as_str)
            .and_then(LspLanguage::from_hint)
            .or_else(|| file_path_opt.as_deref().and_then(LspLanguage::from_path))
            .ok_or_else(|| {
                anyhow!(
                    "Could not infer LSP language; provide 'file_path' with a recognized extension or explicit 'language'"
                )
            })?;

        let workspace_root = file_path_opt
            .as_deref()
            .map(find_workspace_root)
            .unwrap_or_else(|| find_workspace_root(&default_cwd));

        let (server_name, server_args, server_bin) = lang
            .candidate_servers()
            .iter()
            .find_map(|(bin, args)| {
                search_executable(bin, &workspace_root).map(|full| (*bin, *args, full))
            })
            .ok_or_else(|| {
                anyhow!(
                    "No LSP server binary found on PATH for language '{}' (expected one of: {:?})",
                    lang.language_id(),
                    lang.candidate_servers()
                        .iter()
                        .map(|(b, _)| *b)
                        .collect::<Vec<_>>()
                )
            })?;

        let timeout_secs = arguments
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(25)
            .clamp(5, 90);

        let root_uri = path_to_file_uri(&workspace_root)?;
        let line_1idx = arguments
            .get("line")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1);
        let char_1idx = arguments
            .get("character")
            .or_else(|| arguments.get("column"))
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1);
        let query_str = arguments
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let run_future = async {
            let mut cmd = Command::new(&server_bin);
            cmd.args(server_args);
            cmd.current_dir(&workspace_root);
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::null());
            cmd.kill_on_drop(true);

            let mut child = cmd.spawn()?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow!("Failed to capture LSP stdin"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("Failed to capture LSP stdout"))?;
            let mut reader = BufReader::new(stdout);

            let init_req = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": std::process::id(),
                    "rootUri": root_uri,
                    "workspaceFolders": [{
                        "uri": root_uri,
                        "name": workspace_root
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("workspace"),
                    }],
                    "capabilities": {
                        "textDocument": {
                            "hover": { "contentFormat": ["markdown", "plaintext"] },
                            "definition": { "linkSupport": true },
                            "references": {},
                            "documentSymbol": { "hierarchicalDocumentSymbolSupport": true }
                        },
                        "workspace": {
                            "symbol": {}
                        }
                    }
                }
            });
            write_lsp_frame(&mut stdin, &init_req).await?;
            let _init_res = await_lsp_response(&mut reader, &mut stdin, 1).await?;

            write_lsp_frame(
                &mut stdin,
                &json!({
                    "jsonrpc": "2.0",
                    "method": "initialized",
                    "params": {}
                }),
            )
            .await?;

            let file_uri_opt = match file_path_opt.as_ref() {
                Some(file_path) => {
                    file_path
                        .is_file()
                        .then_some(())
                        .ok_or_else(|| anyhow!("Target file does not exist: {:?}", file_path))?;
                    let uri = path_to_file_uri(file_path)?;
                    let source_text = std::fs::read_to_string(file_path)?;
                    write_lsp_frame(
                        &mut stdin,
                        &json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/didOpen",
                            "params": {
                                "textDocument": {
                                    "uri": uri,
                                    "languageId": lang.language_id(),
                                    "version": 1,
                                    "text": source_text,
                                }
                            }
                        }),
                    )
                    .await?;
                    Some(uri)
                }
                None => None,
            };

            let pos = json!({
                "line": line_1idx - 1,
                "character": char_1idx - 1,
            });

            let req_payload = match action.as_str() {
                "hover" => {
                    let uri = file_uri_opt
                        .ok_or_else(|| anyhow!("action='hover' requires 'file_path'"))?;
                    json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "textDocument/hover",
                        "params": {
                            "textDocument": { "uri": uri },
                            "position": pos,
                        }
                    })
                }
                "definition" => {
                    let uri = file_uri_opt
                        .ok_or_else(|| anyhow!("action='definition' requires 'file_path'"))?;
                    json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "textDocument/definition",
                        "params": {
                            "textDocument": { "uri": uri },
                            "position": pos,
                        }
                    })
                }
                "references" => {
                    let uri = file_uri_opt
                        .ok_or_else(|| anyhow!("action='references' requires 'file_path'"))?;
                    json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "textDocument/references",
                        "params": {
                            "textDocument": { "uri": uri },
                            "position": pos,
                            "context": { "includeDeclaration": true },
                        }
                    })
                }
                "document_symbols" => {
                    let uri = file_uri_opt.ok_or_else(|| {
                        anyhow!("action='document_symbols' requires 'file_path'")
                    })?;
                    json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "textDocument/documentSymbol",
                        "params": {
                            "textDocument": { "uri": uri },
                        }
                    })
                }
                "workspace_symbols" => json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "workspace/symbol",
                    "params": {
                        "query": query_str,
                    }
                }),
                other => return Err(anyhow!("Unsupported lsp_query action '{}'", other)),
            };

            write_lsp_frame(&mut stdin, &req_payload).await?;
            let raw_res = await_lsp_response(&mut reader, &mut stdin, 2).await?;

            // Best-effort graceful shutdown
            let _ = write_lsp_frame(
                &mut stdin,
                &json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": Value::Null }),
            )
            .await;
            let _ = write_lsp_frame(
                &mut stdin,
                &json!({ "jsonrpc": "2.0", "method": "exit", "params": Value::Null }),
            )
            .await;

            let formatted = match action.as_str() {
                "hover" => json!({
                    "status": "success",
                    "server": server_name,
                    "hover": extract_hover_markdown(&raw_res),
                }),
                "definition" | "references" => {
                    let locations = normalize_lsp_locations(&raw_res, &workspace_root);
                    json!({
                        "status": "success",
                        "server": server_name,
                        "count": locations.len(),
                        "locations": locations,
                    })
                }
                "document_symbols" | "workspace_symbols" => {
                    let mut symbols = Vec::new();
                    match raw_res.as_array() {
                        Some(arr) => flatten_document_symbols(arr, &mut symbols, &workspace_root),
                        None => {}
                    }
                    json!({
                        "status": "success",
                        "server": server_name,
                        "count": symbols.len(),
                        "symbols": symbols,
                    })
                }
                _ => raw_res,
            };

            Ok::<Value, anyhow::Error>(formatted)
        };

        tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), run_future)
            .await
            .map_err(|_| anyhow!("LSP query '{}' timed out after {}s", action, timeout_secs))?
    }
}

#[cfg(test)]
#[path = "lsp_client_tests.rs"]
mod tests;
