use anyhow::Result;
use openz_docs_mcp::{
    init_db, DocsMcpServer, InstallDocsetRequest, ListDocsetsRequest, ReadDocPageRequest,
    ReadRustDocsRequest, SearchDocsRequest, SearchRustCrateRequest,
};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{json, Value};

pub fn get_server() -> &'static DocsMcpServer {
    static SERVER: std::sync::OnceLock<DocsMcpServer> = std::sync::OnceLock::new();
    SERVER.get_or_init(|| {
        let db_path = crate::config::loader::runtime_db_path("docs.db");
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = init_db(&db_path);
        DocsMcpServer::new(db_path)
    })
}

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
        let schema = schemars::schema_for!(ListDocsetsRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: ListDocsetsRequest = serde_json::from_value(arguments.clone())?;
        match get_server().list_docsets(Parameters(p)).await {
            Ok(res_str) => {
                let val: Value = serde_json::from_str(&res_str).unwrap_or_else(|_| json!(res_str));
                Ok(json!({ "success": true, "result": val }))
            }
            Err(e) => Ok(json!({ "success": false, "error": e.message })),
        }
    }
}

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
        let schema = schemars::schema_for!(InstallDocsetRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: InstallDocsetRequest = serde_json::from_value(arguments.clone())?;
        match get_server().install_docset(Parameters(p)).await {
            Ok(res_str) => Ok(json!({ "success": true, "result": res_str })),
            Err(e) => Ok(json!({ "success": false, "error": e.message })),
        }
    }
}

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
        let schema = schemars::schema_for!(SearchDocsRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: SearchDocsRequest = serde_json::from_value(arguments.clone())?;
        match get_server().search_docs(Parameters(p)).await {
            Ok(res_str) => {
                let val: Value = serde_json::from_str(&res_str).unwrap_or_else(|_| json!(res_str));
                Ok(json!({ "success": true, "result": val }))
            }
            Err(e) => Ok(json!({ "success": false, "error": e.message })),
        }
    }
}

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
        let schema = schemars::schema_for!(ReadDocPageRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: ReadDocPageRequest = serde_json::from_value(arguments.clone())?;
        match get_server().read_doc_page(Parameters(p)).await {
            Ok(res_str) => Ok(json!({ "success": true, "result": res_str })),
            Err(e) => Ok(json!({ "success": false, "error": e.message })),
        }
    }
}

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
        let schema = schemars::schema_for!(SearchRustCrateRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: SearchRustCrateRequest = serde_json::from_value(arguments.clone())?;
        match get_server().search_rust_crate(Parameters(p)).await {
            Ok(res_str) => {
                let val: Value = serde_json::from_str(&res_str).unwrap_or_else(|_| json!(res_str));
                Ok(json!({ "success": true, "result": val }))
            }
            Err(e) => Ok(json!({ "success": false, "error": e.message })),
        }
    }
}

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
        let schema = schemars::schema_for!(ReadRustDocsRequest);
        serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let p: ReadRustDocsRequest = serde_json::from_value(arguments.clone())?;
        match get_server().read_rust_docs(Parameters(p)).await {
            Ok(res_str) => Ok(json!({ "success": true, "result": res_str })),
            Err(e) => Ok(json!({ "success": false, "error": e.message })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_init() {
        let server = get_server();
        assert!(server.get_db_conn().is_ok());
    }
}
