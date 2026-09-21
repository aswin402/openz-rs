//! Native documentation and crate inspection tools.
//!
//! Provides in-process docset management, DevDocs querying, crates.io searching,
//! and docs.rs rendering backed by OpenZ's local SQLite documentation cache (`docs.db`).

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const USER_AGENT: &str = "OpenZ-Agent (aswin@openz.ai)";

/// Information about an installed DevDocs documentation set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocsetInfo {
    pub name: String,
    pub last_updated: String,
}

/// Search result from an installed docset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocSearchResult {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
}

/// Information about a crate from crates.io.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub documentation: String,
}

#[derive(Deserialize)]
struct DevDocsEntry {
    name: String,
    path: String,
    #[serde(rename = "type")]
    entry_type: Option<String>,
}

#[derive(Deserialize)]
struct DevDocsIndex {
    entries: Vec<DevDocsEntry>,
}

/// Ensure documentation tables exist and WAL mode is enabled.
pub fn init_docs_db(db_path: &Path) -> Result<(), rusqlite::Error> {
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(db_path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS docsets (
             name TEXT PRIMARY KEY,
             last_updated DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS entries (
             docset TEXT,
             name TEXT,
             path TEXT,
             type TEXT,
             PRIMARY KEY(docset, path)
         );
         CREATE TABLE IF NOT EXISTS contents (
             docset TEXT,
             path TEXT,
             html TEXT,
             markdown TEXT,
             PRIMARY KEY(docset, path)
         );",
    )?;
    Ok(())
}

/// Helper to get a SQLite connection for the runtime `docs.db`.
pub fn get_docs_db_conn() -> Result<Connection, rusqlite::Error> {
    let db_path = crate::config::loader::runtime_db_path("docs.db");
    init_docs_db(&db_path)?;
    Connection::open(db_path)
}

/// Native documentation service providing docset queries and Rust crate doc retrieval.
#[derive(Clone, Debug)]
pub struct DocsService {
    db_path: PathBuf,
}

impl Default for DocsService {
    fn default() -> Self {
        Self::new(crate::config::loader::runtime_db_path("docs.db"))
    }
}

impl DocsService {
    pub fn new(db_path: PathBuf) -> Self {
        let _ = init_docs_db(&db_path);
        Self { db_path }
    }

    pub fn get_db_conn(&self) -> Result<Connection, rusqlite::Error> {
        init_docs_db(&self.db_path)?;
        Connection::open(&self.db_path)
    }

    pub fn list_docsets(&self) -> Result<Vec<DocsetInfo>> {
        let conn = self.get_db_conn()?;
        let mut stmt = conn.prepare("SELECT name, last_updated FROM docsets ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(DocsetInfo {
                name: row.get(0)?,
                last_updated: row.get(1)?,
            })
        })?;

        let mut result = Vec::new();
        for info in rows.flatten() {
            result.push(info);
        }
        Ok(result)
    }

    pub async fn install_docset(&self, docset_name: &str) -> Result<String> {
        let docset = docset_name.trim().to_lowercase();
        if docset.is_empty() {
            anyhow::bail!("Docset name cannot be empty.");
        }

        let index_url = format!("https://documents.devdocs.io/{}/index.json", docset);
        let db_url = format!("https://documents.devdocs.io/{}/db.json", docset);

        crate::tools::web::validate_url(&index_url)
            .await
            .map_err(|e| anyhow::anyhow!("SSRF safety violation for index URL: {e}"))?;
        crate::tools::web::validate_url(&db_url)
            .await
            .map_err(|e| anyhow::anyhow!("SSRF safety violation for database URL: {e}"))?;

        let client = crate::core::http::default_http_client();

        let index_resp = client
            .get(&index_url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch index.json: {}", e))?;

        if !index_resp.status().is_success() {
            anyhow::bail!(
                "Library '{}' not found or failed to fetch index (HTTP {}).",
                docset,
                index_resp.status()
            );
        }

        let devdocs_index: DevDocsIndex = index_resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse index.json: {}", e))?;

        let db_resp = client
            .get(&db_url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch db.json: {}", e))?;

        if !db_resp.status().is_success() {
            anyhow::bail!(
                "Failed to fetch database content for '{}' (HTTP {}).",
                docset,
                db_resp.status()
            );
        }

        let devdocs_db: HashMap<String, String> = db_resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse db.json: {}", e))?;

        let mut conn = self.get_db_conn()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT OR REPLACE INTO docsets (name, last_updated) VALUES (?, CURRENT_TIMESTAMP)",
            [&docset],
        )?;

        {
            let mut entry_stmt = tx.prepare(
                "INSERT OR REPLACE INTO entries (docset, name, path, type) VALUES (?, ?, ?, ?)",
            )?;
            for entry in &devdocs_index.entries {
                entry_stmt.execute([
                    &docset,
                    &entry.name,
                    &entry.path,
                    entry.entry_type.as_deref().unwrap_or(""),
                ])?;
            }
        }

        {
            let mut content_stmt = tx.prepare(
                "INSERT OR REPLACE INTO contents (docset, path, html, markdown) VALUES (?, ?, ?, NULL)",
            )?;
            for (path, html) in &devdocs_db {
                content_stmt.execute([&docset, path, html])?;
            }
        }

        tx.commit()?;

        Ok(format!(
            "Successfully installed docset '{}' with {} pages and entries.",
            docset,
            devdocs_index.entries.len()
        ))
    }

    pub fn search_docs(&self, docset_name: &str, query: &str) -> Result<Vec<DocSearchResult>> {
        let docset = docset_name.trim().to_lowercase();
        let query_trimmed = query.trim();
        if docset.is_empty() || query_trimmed.is_empty() {
            anyhow::bail!("Docset name and query cannot be empty.");
        }

        let conn = self.get_db_conn()?;
        let mut stmt = conn.prepare(
            "SELECT name, path, type FROM entries 
             WHERE docset = ? AND (name LIKE ? OR path LIKE ?) 
             LIMIT 30",
        )?;

        let query_pattern = format!("%{}%", query_trimmed);
        let rows = stmt.query_map([&docset, &query_pattern, &query_pattern], |row| {
            Ok(DocSearchResult {
                name: row.get(0)?,
                path: row.get(1)?,
                entry_type: row.get(2)?,
            })
        })?;

        let mut result = Vec::new();
        for res in rows.flatten() {
            result.push(res);
        }
        Ok(result)
    }

    pub fn read_doc_page(&self, docset_name: &str, path: &str) -> Result<String> {
        let docset = docset_name.trim().to_lowercase();
        let path_trimmed = path.trim();

        if docset.is_empty() || path_trimmed.is_empty() {
            anyhow::bail!("Docset name and path cannot be empty.");
        }

        let conn = self.get_db_conn()?;
        let mut stmt = conn.prepare("SELECT html, markdown FROM contents WHERE docset = ? AND path = ?")?;

        let mut rows = stmt.query_map([&docset, path_trimmed], |row| {
            let html: Option<String> = row.get(0).ok();
            let markdown: Option<String> = row.get(1).ok();
            Ok((html, markdown))
        })?;

        if let Some(first) = rows.next() {
            let (html, markdown) = first?;
            if let Some(md) = markdown {
                if !md.is_empty() {
                    return Ok(md);
                }
            }
            if let Some(h) = html {
                let md = html2md::parse_html(&h);
                let _ = conn.execute(
                    "UPDATE contents SET markdown = ? WHERE docset = ? AND path = ?",
                    [&md, &docset, path_trimmed],
                );
                return Ok(md);
            }
        }

        anyhow::bail!(
            "Documentation page '{}' not found in docset '{}'. Verify you've installed it via docs_install_docset.",
            path_trimmed, docset
        )
    }

    pub async fn search_rust_crate(&self, query: &str) -> Result<Vec<CrateInfo>> {
        let query_trimmed = query.trim();
        if query_trimmed.is_empty() {
            anyhow::bail!("Query cannot be empty.");
        }

        let encoded_query =
            percent_encoding::utf8_percent_encode(query_trimmed, percent_encoding::NON_ALPHANUMERIC);
        let url = format!(
            "https://crates.io/api/v1/crates?q={}&per_page=5",
            encoded_query
        );

        crate::tools::web::validate_url(&url)
            .await
            .map_err(|e| anyhow::anyhow!("SSRF safety violation: {e}"))?;

        let client = crate::core::http::default_http_client();
        let resp = client
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to search crates.io: {}", e))?;

        if !resp.status().is_success() {
            anyhow::bail!("Crates.io returned HTTP {}", resp.status());
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse crates.io response: {}", e))?;

        let mut results = Vec::new();
        if let Some(arr) = data.get("crates").and_then(|v| v.as_array()) {
            for item in arr {
                results.push(CrateInfo {
                    name: item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    version: item.get("max_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    description: item.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    documentation: item.get("documentation").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                });
            }
        }

        Ok(results)
    }

    pub async fn read_rust_docs(&self, crate_name: &str, sub_path: Option<&str>) -> Result<String> {
        let crate_trimmed = crate_name.trim();
        if crate_trimmed.is_empty() {
            anyhow::bail!("Crate name cannot be empty.");
        }

        let sub_path_actual = sub_path.unwrap_or("index.html").trim();
        let db_cache_path = format!("rust_docs/{}/{}", crate_trimmed, sub_path_actual);

        // Check SQLite cache
        if let Ok(conn) = self.get_db_conn() {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT markdown FROM contents WHERE docset = 'rust-crates' AND path = ?",
            ) {
                if let Ok(mut rows) = stmt.query_map([&db_cache_path], |row| {
                    let markdown: Option<String> = row.get(0).ok();
                    Ok(markdown)
                }) {
                    if let Some(Ok(Some(md))) = rows.next() {
                        if !md.is_empty() {
                            return Ok(md);
                        }
                    }
                }
            }
        }

        let module_name = crate_trimmed.replace('-', "_");
        let url = if sub_path_actual.starts_with("http://") || sub_path_actual.starts_with("https://") {
            sub_path_actual.to_string()
        } else {
            format!(
                "https://docs.rs/{}/latest/{}/{}",
                crate_trimmed, module_name, sub_path_actual
            )
        };

        crate::tools::web::validate_url(&url)
            .await
            .map_err(|e| anyhow::anyhow!("SSRF safety violation: {e}"))?;

        let client = crate::core::http::default_http_client();
        let resp_result = client
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await;

        let resp = match resp_result {
            Ok(r) if r.status().is_success() => r,
            _ => {
                let fallback_url = format!("https://docs.rs/{}/latest/{}/", crate_trimmed, module_name);
                client
                    .get(&fallback_url)
                    .header(reqwest::header::USER_AGENT, USER_AGENT)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to connect to docs.rs: {}", e))?
            }
        };

        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to retrieve docs from docs.rs for {} (HTTP {})",
                crate_trimmed,
                resp.status()
            );
        }

        let html_content = resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read docs.rs HTML body: {}", e))?;

        let fragment = scraper::Html::parse_document(&html_content);
        let main_selectors = ["main", "#main-content", ".content"];
        let mut main_html = html_content.clone();

        for selector_str in main_selectors {
            if let Ok(selector) = scraper::Selector::parse(selector_str) {
                if let Some(el) = fragment.select(&selector).next() {
                    main_html = el.html();
                    break;
                }
            }
        }

        let markdown = html2md::parse_html(&main_html);

        if let Ok(conn) = self.get_db_conn() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO contents (docset, path, html, markdown) VALUES ('rust-crates', ?, NULL, ?)",
                [&db_cache_path, &markdown],
            );
        }

        Ok(markdown)
    }
}

/// Global shared instance of `DocsService`.
pub fn global_docs_service() -> &'static DocsService {
    static SERVICE: OnceLock<DocsService> = OnceLock::new();
    SERVICE.get_or_init(DocsService::default)
}

// ── Native OpenZ Tool Implementations ──

/// Tool: List all locally installed documentation sets.
pub struct DocsListDocsetsTool;

#[async_trait::async_trait]
impl crate::tools::Tool for DocsListDocsetsTool {
    fn name(&self) -> &str {
        "docs_list_docsets"
    }

    fn description(&self) -> &str {
        "List all locally installed documentation sets."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let service = global_docs_service();
        match service.list_docsets() {
            Ok(list) => {
                if list.is_empty() {
                    Ok(json!({
                        "success": true,
                        "result": "No documentation sets are currently installed. Use docs_install_docset to download one (e.g. 'react', 'python', 'javascript', 'css', 'zod', 'prisma', 'rust')."
                    }))
                } else {
                    Ok(json!({ "success": true, "result": list }))
                }
            }
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
    }
}

/// Tool: Download and install a documentation set from DevDocs.io index.
pub struct DocsInstallDocsetTool;

#[async_trait::async_trait]
impl crate::tools::Tool for DocsInstallDocsetTool {
    fn name(&self) -> &str {
        "docs_install_docset"
    }

    fn description(&self) -> &str {
        "Download and install a documentation set from DevDocs.io index."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "docset_name": {
                    "type": "string",
                    "description": "The name of the docset to install (e.g. 'react', 'python', 'javascript', 'css', 'zod', 'prisma', 'rust')."
                }
            },
            "required": ["docset_name"],
            "additionalProperties": true
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let docset_name = arguments
            .get("docset_name")
            .or_else(|| arguments.get("docsetName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if docset_name.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "Missing required parameter 'docset_name'."
            }));
        }

        let service = global_docs_service();
        match service.install_docset(docset_name).await {
            Ok(msg) => Ok(json!({ "success": true, "result": msg })),
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
    }
}

/// Tool: Search for articles, methods, or classes inside a specific documentation set.
pub struct DocsSearchDocsTool;

#[async_trait::async_trait]
impl crate::tools::Tool for DocsSearchDocsTool {
    fn name(&self) -> &str {
        "docs_search_docs"
    }

    fn description(&self) -> &str {
        "Search for articles, methods, or classes inside a specific documentation set."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "docset_name": {
                    "type": "string",
                    "description": "The docset name (e.g. 'react', 'python')."
                },
                "query": {
                    "type": "string",
                    "description": "The search query (e.g. 'useState', 'list.append')."
                }
            },
            "required": ["docset_name", "query"],
            "additionalProperties": true
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let docset_name = arguments
            .get("docset_name")
            .or_else(|| arguments.get("docsetName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let query = arguments
            .get("query")
            .or_else(|| arguments.get("search_query"))
            .or_else(|| arguments.get("q"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if docset_name.is_empty() || query.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "Both 'docset_name' and 'query' parameters are required."
            }));
        }

        let service = global_docs_service();
        match service.search_docs(docset_name, query) {
            Ok(results) => {
                if results.is_empty() {
                    Ok(json!({
                        "success": true,
                        "result": format!("No results found for query '{}' in docset '{}'.", query, docset_name)
                    }))
                } else {
                    Ok(json!({ "success": true, "result": results }))
                }
            }
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
    }
}

/// Tool: Read a specific documentation page in clean Markdown.
pub struct DocsReadDocPageTool;

#[async_trait::async_trait]
impl crate::tools::Tool for DocsReadDocPageTool {
    fn name(&self) -> &str {
        "docs_read_doc_page"
    }

    fn description(&self) -> &str {
        "Read a specific documentation page. Renders the content in clean Markdown."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "docset_name": {
                    "type": "string",
                    "description": "The docset name (e.g. 'react', 'python')."
                },
                "path": {
                    "type": "string",
                    "description": "The path of the page to read (e.g. 'react/hooks-reference.html')."
                }
            },
            "required": ["docset_name", "path"],
            "additionalProperties": true
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let docset_name = arguments
            .get("docset_name")
            .or_else(|| arguments.get("docsetName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let path = arguments
            .get("path")
            .or_else(|| arguments.get("page_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if docset_name.is_empty() || path.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "Both 'docset_name' and 'path' parameters are required."
            }));
        }

        let service = global_docs_service();
        match service.read_doc_page(docset_name, path) {
            Ok(content) => Ok(json!({ "success": true, "result": content })),
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
    }
}

/// Tool: Search for a crate on crates.io.
pub struct DocsSearchRustCrateTool;

#[async_trait::async_trait]
impl crate::tools::Tool for DocsSearchRustCrateTool {
    fn name(&self) -> &str {
        "docs_search_rust_crate"
    }

    fn description(&self) -> &str {
        "Search for a crate on crates.io to find its description and latest version."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The crate name or query to search on crates.io (e.g. 'tokio', 'serde')."
                }
            },
            "required": ["query"],
            "additionalProperties": true
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .or_else(|| arguments.get("q"))
            .or_else(|| arguments.get("crate_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if query.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "Missing required parameter 'query'."
            }));
        }

        let service = global_docs_service();
        match service.search_rust_crate(query).await {
            Ok(results) => Ok(json!({ "success": true, "result": results })),
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
    }
}

/// Tool: Read documentation for any third-party Rust crate from docs.rs.
pub struct DocsReadRustDocsTool;

#[async_trait::async_trait]
impl crate::tools::Tool for DocsReadRustDocsTool {
    fn name(&self) -> &str {
        "docs_read_rust_docs"
    }

    fn description(&self) -> &str {
        "Read documentation for any third-party Rust crate from docs.rs."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "crate_name": {
                    "type": "string",
                    "description": "The name of the Rust crate (e.g. 'tokio', 'serde')."
                },
                "sub_path": {
                    "type": "string",
                    "description": "Optional specific sub-path or item (e.g. 'struct.HashMap.html', 'fn.spawn.html')."
                }
            },
            "required": ["crate_name"],
            "additionalProperties": true
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let crate_name = arguments
            .get("crate_name")
            .or_else(|| arguments.get("crateName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let sub_path = arguments
            .get("sub_path")
            .or_else(|| arguments.get("subPath"))
            .and_then(|v| v.as_str());

        if crate_name.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "Missing required parameter 'crate_name'."
            }));
        }

        let service = global_docs_service();
        match service.read_rust_docs(crate_name, sub_path).await {
            Ok(content) => Ok(json!({ "success": true, "result": content })),
            Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
        }
    }
}

#[cfg(test)]
#[path = "docs_mcp_tests.rs"]
mod tests;
