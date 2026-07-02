pub mod scoping;
pub mod stats;
pub mod compress;
pub mod cache;

// Re-exports
pub use scoping::{ScopeContextTool, SummarizeCodebaseTool};
pub use stats::{CountTokensTool, PingTool, ServerInfoTool};
pub use compress::{
    CompressContentTool, RetrieveOriginalTool, CompressSchemaTool, CompressFileTool,
    CompressDiffTool, CompressUrlTool, RunAndCompressTool, CompressDirectoryTool,
};
pub use cache::{
    CacheStatsTool, ClearCacheTool, SearchCacheTool, CacheAlignTool, ExportCacheTool,
    ImportCacheTool,
};

// Shared Constants and Helpers
pub const MAX_INPUT_SIZE: usize = 512_000; // 500KB max input
pub const CACHE_CAPACITY: usize = 1000;

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() { return 0; }
    (text.len() + 3) / 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use serde_json::json;
    use crate::tools::graph_memory::test_lock;
    use crate::tools::Tool;

    #[tokio::test]
    async fn test_auto_detect_json() {
        assert_eq!(compress::auto_detect_type(r#"{"key": "value"}"#), "json");
        assert_eq!(compress::auto_detect_type("[1, 2, 3]"), "json");
    }

    #[tokio::test]
    async fn test_auto_detect_code() {
        assert_eq!(compress::auto_detect_type("fn main() { println!(\"hi\"); }"), "code");
        assert_eq!(compress::auto_detect_type("def hello():\n    pass"), "code");
    }

    #[tokio::test]
    async fn test_estimate_tokens_basic() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[tokio::test]
    async fn test_compress_content_preview() {
        let tool = CompressContentTool;
        let res = tool.call(&json!({
            "raw_text": "fn hello() { println!(\"world\"); }",
            "content_type": "code",
            "preview": true
        })).await.unwrap();
        assert!(res["compressed"].as_str().unwrap().contains("hello"));
        assert!(res["ccr_id"].is_null());
    }

    #[tokio::test]
    async fn test_compress_content_then_retrieve() {
        let _l = test_lock().lock().await;

        let tool_c = CompressContentTool;
        let res = tool_c.call(&json!({
            "raw_text": "This is a test string for CCR round-trip verification.",
            "content_type": "text_logs",
            "preview": false
        })).await.unwrap();

        let ccr_id = res["ccr_id"].as_str().unwrap().to_string();
        assert!(ccr_id.starts_with("ccr_"));
        assert!(res["compressed_tokens"].as_u64().unwrap() > 0);

        let tool_r = RetrieveOriginalTool;
        let res2 = tool_r.call(&json!({ "ccr_id": ccr_id })).await.unwrap();
        assert_eq!(res2["content"].as_str().unwrap(), "This is a test string for CCR round-trip verification.");
        assert_eq!(res2["source"], "cache");
    }

    #[tokio::test]
    async fn test_retrieve_original_missing() {
        let tool = RetrieveOriginalTool;
        let res = tool.call(&json!({ "ccr_id": "ccr_nonexistent_123" })).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_compress_json_content() {
        let tool = CompressContentTool;
        let json_input = r#"{"name":"test","items":[1,2,3],"nested":{"key":"val"}}"#;
        let res = tool.call(&json!({ "raw_text": json_input, "content_type": "json", "preview": true })).await.unwrap();
        let compressed = res["compressed"].as_str().unwrap();
        assert!(compressed.contains("name") || compressed.contains("items"));
    }

    #[tokio::test]
    async fn test_ping() {
        let tool = PingTool;
        let res = tool.call(&json!({})).await.unwrap();
        assert_eq!(res["status"], "ok");
    }

    #[tokio::test]
    async fn test_server_info() {
        let tool = ServerInfoTool;
        let res = tool.call(&json!({})).await.unwrap();
        assert!(res["cache_size"].as_u64().is_some());
        assert_eq!(res["cache_capacity"], CACHE_CAPACITY);
    }

    #[tokio::test]
    async fn test_count_tokens() {
        let tool = CountTokensTool;
        let res = tool.call(&json!({ "text": "hello world" })).await.unwrap();
        assert_eq!(res["tokens"], 3);
        assert_eq!(res["characters"], 11);
    }

    #[tokio::test]
    async fn test_cache_clear_and_stats() {
        let _l = test_lock().lock().await;

        // First insert something
        let tool_c = CompressContentTool;
        let _ = tool_c.call(&json!({
            "raw_text": "cache test data for stats",
            "content_type": "text_logs",
            "preview": false
        })).await.unwrap();

        // Check stats
        let stats = CacheStatsTool;
        let res = stats.call(&json!({})).await.unwrap();
        assert!(res["total_items"].as_u64().unwrap() > 0);

        // Clear
        let clear = ClearCacheTool;
        let res2 = clear.call(&json!({})).await.unwrap();
        assert!(res2["evicted"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_search_cache() {
        let _l = test_lock().lock().await;

        let tool_c = CompressContentTool;
        let _ = tool_c.call(&json!({
            "raw_text": "unique_search_term_for_testing_12345",
            "content_type": "text_logs",
            "preview": false
        })).await.unwrap();

        let search = SearchCacheTool;
        let res = search.call(&json!({ "query": "unique_search_term" })).await.unwrap();
        assert!(res["count"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_cache_align() {
        let tool = CacheAlignTool;
        let res = tool.call(&json!({
            "chunks": ["chunk b", "chunk a"],
            "padding_size": 16
        })).await.unwrap();

        let aligned = res["aligned"].as_str().unwrap();
        assert!(aligned.find("chunk a").unwrap() < aligned.find("chunk b").unwrap());
        assert!(aligned.contains("<!-- chunk: "));
    }

    #[tokio::test]
    async fn test_compress_schema() {
        let tool = CompressSchemaTool;
        let schema = r#"{ "title": "Test", "description": "A test tool", "properties": { "name": { "type": "string", "description": "Name" } } }"#;
        let res = tool.call(&json!({ "schema": schema })).await.unwrap();
        let compressed = res["schema"].as_str().unwrap();
        assert!(!compressed.contains("description"));
        assert!(!compressed.contains("title"));
        assert!(compressed.contains("name"));
    }

    #[tokio::test]
    async fn test_compress_diff() {
        let tool = CompressDiffTool;
        let diff = r#"diff --git a/src/server.rs b/src/server.rs
--- a/src/server.rs
+++ b/src/server.rs
@@ -10,3 +10,4 @@ fn my_func()
-old
+new
"#;
        let res = tool.call(&json!({ "diff_text": diff, "preview": true })).await.unwrap();
        let compressed = res["compressed"].as_str().unwrap();
        assert!(compressed.contains("Diff Summary"));
        assert!(compressed.contains("src/server.rs"));
    }

    #[tokio::test]
    async fn test_compress_file() {
        let _l = test_lock().lock().await;
        let dir = std::env::temp_dir().join("headroom_test_compress_file");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test_hello.rs");
        std::fs::write(&file_path, "fn test() { println!(\"hello\"); }").unwrap();

        let tool = CompressFileTool;
        let res = tool.call(&json!({
            "file_path": file_path.to_string_lossy(),
            "content_type": "code",
            "preview": true
        })).await.unwrap();
        assert!(res["compressed"].as_str().unwrap().contains("test"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_export_import_cache() {
        let _l = test_lock().lock().await;

        // Insert test data
        let cid = cache::cache_content("export_test_data").unwrap();
        assert!(cid.starts_with("ccr_"));

        let dir = std::env::temp_dir().join("headroom_test_export");
        std::fs::create_dir_all(&dir).unwrap();
        let export_path = dir.join("cache_export.json");

        // Export
        let export = ExportCacheTool;
        let res = export.call(&json!({ "file_path": export_path.to_string_lossy() })).await.unwrap();
        assert!(res["count"].as_u64().unwrap() > 0);

        // Clear cache
        let clear = ClearCacheTool;
        let _ = clear.call(&json!({})).await.unwrap();

        // Import
        let import = ImportCacheTool;
        let res2 = import.call(&json!({ "file_path": export_path.to_string_lossy() })).await.unwrap();
        assert!(res2["imported"].as_u64().unwrap() > 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_detect_content_type_from_ext() {
        assert_eq!(compress::detect_content_type_from_ext(Path::new("test.rs")), Some("code"));
        assert_eq!(compress::detect_content_type_from_ext(Path::new("data.json")), Some("json"));
        assert_eq!(compress::detect_content_type_from_ext(Path::new("doc.md")), Some("markdown"));
        assert_eq!(compress::detect_content_type_from_ext(Path::new("unknown.xyz")), None);
    }

    #[tokio::test]
    async fn test_detect_project_type() {
        let dir = std::env::temp_dir().join("headroom_test_projtype");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "").unwrap();
        assert_eq!(scoping::detect_project_type(&dir), "Rust");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_summarize_codebase() {
        let _l = test_lock().lock().await;
        let dir = std::env::temp_dir().join("headroom_test_codebase_summary");
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(src.join("main.rs"), "fn main() {\n    println!(\"hello\");\n}").unwrap();

        let tool = SummarizeCodebaseTool;
        let res = tool.call(&json!({ "root_path": dir.to_string_lossy() })).await.unwrap();
        assert_eq!(res["project_type"], "Rust");
        assert!(res["total_files"].as_u64().unwrap() >= 2);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_compress_csv_content() {
        let csv = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,SF\nDiana,28,Chicago";
        let result = compress::compress_csv(csv);
        assert!(result.contains("Headers: name,age,city"));
        assert!(result.contains("Row 1:"));
        assert!(result.contains("4 rows total"));
    }

    #[tokio::test]
    async fn test_detect_project_type_variants() {
        let dir = std::env::temp_dir().join("headroom_test_projvar");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(scoping::detect_project_type(&dir), "Unknown");
        std::fs::write(dir.join("package.json"), "{}").unwrap();
        assert_eq!(scoping::detect_project_type(&dir), "Node.js");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_is_binary_file() {
        let dir = std::env::temp_dir().join("headroom_test_binary");
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("test.bin");
        std::fs::write(&bin, b"Hello \x00 world").unwrap();
        assert!(compress::is_binary_file(&bin));
        std::fs::write(&bin, b"Hello world").unwrap();
        assert!(!compress::is_binary_file(&bin));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn test_parse_simple_git_diff() {
        let diff = "diff --git a/src/server.rs b/src/server.rs\n--- a/src/server.rs\n+++ b/src/server.rs\n@@ -10,3 +10,4 @@ fn my_func()\n line1\n line2\n-old_line\n+new_line\n";
        let summary = compress::parse_unified_diff(diff);
        assert_eq!(summary.files.len(), 1);
        assert_eq!(summary.files[0].path, "src/server.rs");
        assert_eq!(summary.files[0].insertions, 1);
        assert_eq!(summary.files[0].deletions, 1);
        assert_eq!(summary.files[0].hunks_count, 1);
        assert!(!summary.files[0].is_binary);
    }

    #[tokio::test]
    async fn test_filter_cargo_output() {
        let raw = "Compiling foo v0.1.0\nwarning: unused variable\nwarning: another warning\nwarning: third\nwarning: fourth\nwarning: fifth\nwarning: sixth\nFinished\n";
        let filtered = compress::filter_cargo_output(raw);
        assert!(!filtered.contains("Compiling foo"));
        assert!(filtered.contains("warning: unused variable"));
        assert!(filtered.contains("more warnings omitted"));
    }

    #[tokio::test]
    async fn test_filter_git_output() {
        let raw = "Enumerating objects: 5\nCounting objects: 100%\nCompressing objects: 100%\nSome real output\n";
        let filtered = compress::filter_git_output(raw);
        assert!(!filtered.contains("Enumerating objects:"));
        assert!(filtered.contains("Some real output"));
    }

    #[tokio::test]
    async fn test_filter_python_output() {
        let raw = "Collecting requests\nDownloading requests-2.28.0-py3-none-any.whl\nreal output here\n";
        let filtered = compress::filter_python_output(raw);
        assert!(!filtered.contains("Collecting requests"));
        assert!(filtered.contains("real output here"));
    }

    #[tokio::test]
    async fn test_scope_context_yagni_enabled() {
        let _l = test_lock().lock().await;
        std::env::set_var("HEADROOM_ENFORCE_YAGNI", "true");
        let temp_dir = std::env::temp_dir().join("headroom_test_yagni_enabled");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("AGENTS.md"), "test content").unwrap();

        let req = json!({ "target_path": temp_dir.to_str().unwrap() });
        let res = ScopeContextTool.call(&req).await.unwrap();
        let content = res["content"].as_str().unwrap();
        assert!(content.contains("YAGNI Minimalism Directives"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        std::env::remove_var("HEADROOM_ENFORCE_YAGNI");
    }

    #[tokio::test]
    async fn test_scope_context_yagni_disabled() {
        let _l = test_lock().lock().await;
        std::env::remove_var("HEADROOM_ENFORCE_YAGNI");
        let temp_dir = std::env::temp_dir().join("headroom_test_yagni_disabled");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("AGENTS.md"), "test content").unwrap();

        let req = json!({ "target_path": temp_dir.to_str().unwrap() });
        let res = ScopeContextTool.call(&req).await.unwrap();
        let content = res["content"].as_str().unwrap();
        assert!(!content.contains("YAGNI"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
