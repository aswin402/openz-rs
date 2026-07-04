use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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

#[derive(Clone)]
pub struct DocsMcpServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    db_path: PathBuf,
}

fn mcp_error<E: std::fmt::Display>(err: E) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(err.to_string(), None)
}

#[tool_router(server_handler)]
impl DocsMcpServer {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            tool_router: Self::tool_router(),
            db_path,
        }
    }

    pub fn get_db_conn(&self) -> Result<Connection, rusqlite::Error> {
        Connection::open(&self.db_path)
    }

    #[tool(description = "List all locally installed documentation sets.")]
    pub async fn list_docsets(
        &self,
        _req: Parameters<ListDocsetsRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let conn = self.get_db_conn().map_err(mcp_error)?;
        let mut stmt = conn
            .prepare("SELECT name, last_updated FROM docsets ORDER BY name")
            .map_err(mcp_error)?;

        let rows = stmt
            .query_map([], |row| {
                Ok(DocsetInfo {
                    name: row.get(0)?,
                    last_updated: row.get(1)?,
                })
            })
            .map_err(mcp_error)?;

        let mut result = Vec::new();
        for row in rows {
            if let Ok(info) = row {
                result.push(info);
            }
        }

        if result.is_empty() {
            return Ok("No documentation sets are currently installed. Use install_docset to download one (e.g. 'react', 'python', 'javascript', 'css', 'zod', 'prisma', 'rust').".to_string());
        }

        serde_json::to_string_pretty(&result).map_err(mcp_error)
    }

    #[tool(description = "Download and install a documentation set from DevDocs.io index.")]
    pub async fn install_docset(
        &self,
        req: Parameters<InstallDocsetRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let docset = req.0.docset_name.trim().to_lowercase();
        if docset.is_empty() {
            return Err(rmcp::ErrorData::invalid_params(
                "Docset name cannot be empty.",
                None,
            ));
        }

        // Step 1: Download index.json and db.json
        let index_url = format!("https://documents.devdocs.io/{}/index.json", docset);
        let db_url = format!("https://documents.devdocs.io/{}/db.json", docset);

        let client = reqwest::Client::builder().build().map_err(mcp_error)?;

        let index_resp = client.get(&index_url).send().await.map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to fetch index.json: {}", e), None)
        })?;

        if !index_resp.status().is_success() {
            return Err(rmcp::ErrorData::internal_error(
                format!(
                    "Library '{}' not found or failed to fetch index (HTTP {}).",
                    docset,
                    index_resp.status()
                ),
                None,
            ));
        }

        let devdocs_index: DevDocsIndex = index_resp.json().await.map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to parse index.json: {}", e), None)
        })?;

        let db_resp = client.get(&db_url).send().await.map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to fetch db.json: {}", e), None)
        })?;

        if !db_resp.status().is_success() {
            return Err(rmcp::ErrorData::internal_error(
                format!(
                    "Failed to fetch database content for '{}' (HTTP {}).",
                    docset,
                    db_resp.status()
                ),
                None,
            ));
        }

        let devdocs_db: HashMap<String, String> = db_resp.json().await.map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to parse db.json: {}", e), None)
        })?;

        // Step 2: Store in SQLite database using a transaction
        let mut conn = self.get_db_conn().map_err(mcp_error)?;
        let tx = conn.transaction().map_err(mcp_error)?;

        // Insert docset record
        tx.execute(
            "INSERT OR REPLACE INTO docsets (name, last_updated) VALUES (?, CURRENT_TIMESTAMP)",
            [&docset],
        )
        .map_err(mcp_error)?;

        // Insert entries
        {
            let mut entry_stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO entries (docset, name, path, type) VALUES (?, ?, ?, ?)",
                )
                .map_err(mcp_error)?;
            for entry in &devdocs_index.entries {
                entry_stmt
                    .execute([
                        &docset,
                        &entry.name,
                        &entry.path,
                        entry.entry_type.as_deref().unwrap_or(""),
                    ])
                    .map_err(mcp_error)?;
            }
        }

        // Insert contents
        {
            let mut content_stmt = tx
                .prepare("INSERT OR REPLACE INTO contents (docset, path, html, markdown) VALUES (?, ?, ?, NULL)")
                .map_err(mcp_error)?;
            for (path, html) in &devdocs_db {
                content_stmt
                    .execute([&docset, path, html])
                    .map_err(mcp_error)?;
            }
        }

        tx.commit().map_err(mcp_error)?;

        Ok(format!(
            "Successfully installed docset '{}' with {} pages and entries.",
            docset,
            devdocs_index.entries.len()
        ))
    }

    #[tool(
        description = "Search for articles, methods, or classes inside a specific documentation set."
    )]
    pub async fn search_docs(
        &self,
        req: Parameters<SearchDocsRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let docset = req.0.docset_name.trim().to_lowercase();
        let query = req.0.query.trim();
        if docset.is_empty() || query.is_empty() {
            return Err(rmcp::ErrorData::invalid_params(
                "Docset name and query cannot be empty.",
                None,
            ));
        }

        let conn = self.get_db_conn().map_err(mcp_error)?;
        let mut stmt = conn
            .prepare(
                "SELECT name, path, type FROM entries 
                 WHERE docset = ? AND (name LIKE ? OR path LIKE ?) 
                 LIMIT 30",
            )
            .map_err(mcp_error)?;

        let query_pattern = format!("%{}%", query);
        let rows = stmt
            .query_map([&docset, &query_pattern, &query_pattern], |row| {
                Ok(SearchResult {
                    name: row.get(0)?,
                    path: row.get(1)?,
                    entry_type: row.get(2)?,
                })
            })
            .map_err(mcp_error)?;

        let mut result = Vec::new();
        for row in rows {
            if let Ok(res) = row {
                result.push(res);
            }
        }

        if result.is_empty() {
            return Ok(format!(
                "No results found for query '{}' in docset '{}'.",
                query, docset
            ));
        }

        serde_json::to_string_pretty(&result).map_err(mcp_error)
    }

    #[tool(
        description = "Read a specific documentation page. Renders the content in clean Markdown."
    )]
    pub async fn read_doc_page(
        &self,
        req: Parameters<ReadDocPageRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let docset = req.0.docset_name.trim().to_lowercase();
        let path = req.0.path.trim();

        if docset.is_empty() || path.is_empty() {
            return Err(rmcp::ErrorData::invalid_params(
                "Docset name and path cannot be empty.",
                None,
            ));
        }

        let conn = self.get_db_conn().map_err(mcp_error)?;

        // Check if we already have compiled markdown
        let mut stmt = conn
            .prepare("SELECT html, markdown FROM contents WHERE docset = ? AND path = ?")
            .map_err(mcp_error)?;

        let mut rows = stmt
            .query_map([&docset, path], |row| {
                let html: Option<String> = row.get(0).ok();
                let markdown: Option<String> = row.get(1).ok();
                Ok((html, markdown))
            })
            .map_err(mcp_error)?;

        if let Some(first) = rows.next() {
            let (html, markdown) = first.map_err(mcp_error)?;
            if let Some(md) = markdown {
                return Ok(md);
            }
            if let Some(h) = html {
                // Convert HTML to Markdown
                let md = html2md::parse_html(&h);

                // Cache the markdown conversion in the database
                let _ = conn.execute(
                    "UPDATE contents SET markdown = ? WHERE docset = ? AND path = ?",
                    [&md, &docset, path],
                );
                return Ok(md);
            }
        }

        Err(rmcp::ErrorData::internal_error(
            format!(
                "Documentation page '{}' not found in docset '{}'. Verify you've installed it.",
                path, docset
            ),
            None,
        ))
    }

    #[tool(
        description = "Search for a crate on crates.io to find its description and latest version."
    )]
    pub async fn search_rust_crate(
        &self,
        req: Parameters<SearchRustCrateRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let query = req.0.query.trim();
        if query.is_empty() {
            return Err(rmcp::ErrorData::invalid_params(
                "Query cannot be empty.",
                None,
            ));
        }

        let client = reqwest::Client::builder()
            .user_agent("OpenZ-Agent (aswin@openz.ai)")
            .build()
            .map_err(mcp_error)?;

        let encoded_query =
            percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
        let url = format!(
            "https://crates.io/api/v1/crates?q={}&per_page=5",
            encoded_query
        );
        let resp = client.get(&url).send().await.map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to search crates.io: {}", e), None)
        })?;

        if !resp.status().is_success() {
            return Err(rmcp::ErrorData::internal_error(
                format!("Crates.io returned HTTP {}", resp.status()),
                None,
            ));
        }

        let data: serde_json::Value = resp.json().await.map_err(|e| {
            rmcp::ErrorData::internal_error(
                format!("Failed to parse crates.io JSON response: {}", e),
                None,
            )
        })?;

        let crates = data.get("crates").and_then(|v| v.as_array());

        let mut results = Vec::new();
        if let Some(arr) = crates {
            for item in arr {
                results.push(serde_json::json!({
                    "name": item.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "version": item.get("max_version").and_then(|v| v.as_str()).unwrap_or(""),
                    "description": item.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                    "documentation": item.get("documentation").and_then(|v| v.as_str()).unwrap_or("")
                }));
            }
        }

        serde_json::to_string_pretty(&results).map_err(mcp_error)
    }

    #[tool(description = "Read documentation for any third-party Rust crate from docs.rs.")]
    pub async fn read_rust_docs(
        &self,
        req: Parameters<ReadRustDocsRequest>,
    ) -> Result<String, rmcp::ErrorData> {
        let crate_name = req.0.crate_name.trim();
        if crate_name.is_empty() {
            return Err(rmcp::ErrorData::invalid_params(
                "Crate name cannot be empty.",
                None,
            ));
        }

        let sub_path = req.0.sub_path.as_deref().unwrap_or("index.html");

        // Caching key check in SQLite database!
        let db_cache_path = format!("rust_docs/{}/{}", crate_name, sub_path);

        let cached_markdown = {
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
                                Some(md)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(md) = cached_markdown {
            return Ok(md);
        }

        // Fetch from docs.rs
        let client = reqwest::Client::builder()
            .user_agent("OpenZ-Agent (aswin@openz.ai)")
            .build()
            .map_err(mcp_error)?;

        let module_name = crate_name.replace("-", "_");
        let url = if sub_path.starts_with("http") {
            sub_path.to_string()
        } else {
            format!(
                "https://docs.rs/{}/latest/{}/{}",
                crate_name, module_name, sub_path
            )
        };

        let resp_result = client.get(&url).send().await;

        let resp = match resp_result {
            Ok(r) if r.status().is_success() => r,
            _ => {
                // Fallback to simpler base URL format
                let fallback_url =
                    format!("https://docs.rs/{}/latest/{}/", crate_name, module_name);
                client.get(&fallback_url).send().await.map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed to connect to docs.rs: {}", e),
                        None,
                    )
                })?
            }
        };

        if !resp.status().is_success() {
            return Err(rmcp::ErrorData::internal_error(
                format!(
                    "Failed to retrieve docs from docs.rs for {} (HTTP {})",
                    crate_name,
                    resp.status()
                ),
                None,
            ));
        }

        let html_content = resp.text().await.map_err(|e| {
            rmcp::ErrorData::internal_error(
                format!("Failed to read docs.rs HTML body: {}", e),
                None,
            )
        })?;

        // Extract main content HTML
        let fragment = scraper::Html::parse_document(&html_content);
        let main_selectors = vec!["main", "#main-content", ".content"];
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

        // Cache the result in DB
        if let Ok(conn) = self.get_db_conn() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO contents (docset, path, html, markdown) VALUES ('rust-crates', ?, NULL, ?)",
                [&db_cache_path, &markdown],
            );
        }

        Ok(markdown)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchRustCrateRequest {
    #[schemars(
        description = "The crate name or query to search on crates.io (e.g. 'tokio', 'serde')."
    )]
    pub query: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadRustDocsRequest {
    #[schemars(description = "The name of the Rust crate (e.g. 'tokio', 'serde').")]
    pub crate_name: String,
    #[schemars(
        description = "Optional specific sub-path or item (e.g. 'struct.HashMap.html', 'fn.spawn.html')."
    )]
    pub sub_path: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ListDocsetsRequest {}

#[derive(Serialize, JsonSchema)]
pub struct DocsetInfo {
    name: String,
    last_updated: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct InstallDocsetRequest {
    #[schemars(
        description = "The name of the docset to install (e.g. 'react', 'python', 'javascript', 'css', 'zod', 'prisma', 'rust')."
    )]
    pub docset_name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchDocsRequest {
    #[schemars(description = "The docset name (e.g. 'react', 'python').")]
    pub docset_name: String,
    #[schemars(description = "The search query (e.g. 'useState', 'list.append').")]
    pub query: String,
}

#[derive(Serialize, JsonSchema)]
pub struct SearchResult {
    name: String,
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadDocPageRequest {
    #[schemars(description = "The docset name (e.g. 'react', 'python').")]
    pub docset_name: String,
    #[schemars(description = "The path of the page to read (e.g. 'react/hooks-reference.html').")]
    pub path: String,
}

pub fn init_db(db_path: &std::path::Path) -> Result<(), rusqlite::Error> {
    let conn = Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS docsets (
            name TEXT PRIMARY KEY,
            last_updated DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entries (
            docset TEXT,
            name TEXT,
            path TEXT,
            type TEXT,
            PRIMARY KEY(docset, path)
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS contents (
            docset TEXT,
            path TEXT,
            html TEXT,
            markdown TEXT,
            PRIMARY KEY(docset, path)
        )",
        [],
    )?;
    Ok(())
}
