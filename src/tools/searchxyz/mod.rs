use anyhow::anyhow;
use searchxyz::{
    cache::Cache,
    config::Config,
    crawler::Crawler,
    extractor::ExtractionPipeline,
    graph::KnowledgeGraph,
    index::SearchIndex,
    search::{
        bing::BingBackend, brave::BraveBackend, duckduckgo::DuckDuckGoBackend,
        google::GoogleBackend, searxng::SearXngBackend, SearchBackend, SearchDispatcher,
    },
    tools::SearchXyzServer,
};
use std::sync::{Arc, OnceLock};

pub mod graph;
pub mod index;
pub mod web;

pub use graph::{
    SearchXyzIndexRelationshipTool, SearchXyzQueryGraphTool, SearchXyzReadGithubRepoTool,
};
pub use index::{
    SearchXyzClearIndexTool, SearchXyzDeleteSourceTool, SearchXyzExportResearchTool,
    SearchXyzImportResearchTool, SearchXyzIndexContentTool, SearchXyzListSourcesTool,
    SearchXyzRecallTool,
};
pub use web::{
    SearchXyzBrowserSearchTool, SearchXyzDeepResearchTool, SearchXyzDoctorTool,
    SearchXyzReadUrlTool, SearchXyzSearchAndReadTool, SearchXyzSearchWebTool, SearchXyzSiteMapTool,
};

fn openz_config_dir() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("OPENZ_CONFIG_DIR") {
        return std::path::PathBuf::from(path);
    }

    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".openz")
}

fn apply_openz_embedded_paths(config: &mut Config) {
    let defaults = Config::default();
    let base = openz_config_dir().join("searchxyz");

    if std::env::var_os("SEARCHXYZ_INDEX_PATH").is_none()
        && config.index.path == defaults.index.path
    {
        config.index.path = base.join("index");
    }

    if std::env::var_os("SEARCHXYZ_CACHE_PATH").is_none()
        && config.cache.path == defaults.cache.path
    {
        config.cache.path = base.join("cache.json");
    }
}

pub(crate) fn coerce_number_value(val: &serde_json::Value) -> Option<serde_json::Value> {
    match val {
        serde_json::Value::Number(_) => Some(val.clone()),
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if let Ok(i) = trimmed.parse::<i64>() {
                Some(serde_json::Value::Number(serde_json::Number::from(i)))
            } else if let Ok(u) = trimmed.parse::<u64>() {
                Some(serde_json::Value::Number(serde_json::Number::from(u)))
            } else if let Ok(f) = trimmed.parse::<f64>() {
                serde_json::Number::from_f64(f).map(serde_json::Value::Number)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(crate) fn coerce_numeric_fields(val: &mut serde_json::Value, field_names: &[&str]) {
    if let serde_json::Value::Object(map) = val {
        for &name in field_names {
            if let Some(entry) = map.get(name) {
                if let Some(coerced) = coerce_number_value(entry) {
                    map.insert(name.to_string(), coerced);
                }
            }
        }
    }
}

pub(crate) fn coerce_bool_fields(val: &mut serde_json::Value, field_names: &[&str]) {
    if let serde_json::Value::Object(map) = val {
        for &name in field_names {
            if let Some(entry) = map.get(name) {
                if let serde_json::Value::String(s) = entry {
                    let lower = s.trim().to_lowercase();
                    if lower == "true" || lower == "1" || lower == "yes" || lower == "on" {
                        map.insert(name.to_string(), serde_json::Value::Bool(true));
                    } else if lower == "false" || lower == "0" || lower == "no" || lower == "off" {
                        map.insert(name.to_string(), serde_json::Value::Bool(false));
                    }
                } else if let serde_json::Value::Number(n) = entry {
                    if let Some(i) = n.as_i64() {
                        if i == 1 {
                            map.insert(name.to_string(), serde_json::Value::Bool(true));
                        } else if i == 0 {
                            map.insert(name.to_string(), serde_json::Value::Bool(false));
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn map_field_alias(val: &mut serde_json::Value, canonical: &str, aliases: &[&str]) {
    if let serde_json::Value::Object(map) = val {
        let has_canonical = map.get(canonical).is_some_and(|v| !v.is_null());
        if !has_canonical {
            for &alias in aliases {
                if let Some(v) = map.get(alias) {
                    if !v.is_null() {
                        let cloned = v.clone();
                        map.insert(canonical.to_string(), cloned);
                        break;
                    }
                }
            }
        }
    }
}

pub(crate) fn normalize_query_arg(val: &serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => {
            serde_json::json!({ "query": s.trim() })
        }
        serde_json::Value::Object(map) => {
            let mut normalized = val.clone();
            let has_valid_query = map
                .get("query")
                .and_then(|v| v.as_str())
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);

            if !has_valid_query {
                let aliases = ["q", "search", "prompt", "term", "keywords", "text"];
                for alias in aliases {
                    if let Some(v) = map.get(alias).and_then(|v| v.as_str()) {
                        if !v.trim().is_empty() {
                            normalized["query"] = serde_json::Value::String(v.trim().to_string());
                            break;
                        }
                    }
                }
            }
            normalized
        }
        _ => val.clone(),
    }
}

pub(crate) fn normalize_url_arg(val: &serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => {
            let norm_url = crate::tools::web::normalize_web_url(s);
            serde_json::json!({ "url": norm_url })
        }
        serde_json::Value::Object(map) => {
            let mut normalized = val.clone();
            let mut url_val = map
                .get("url")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            if url_val.is_none() {
                let aliases = [
                    "target_url",
                    "targetUrl",
                    "uri",
                    "link",
                    "target",
                    "href",
                    "page",
                    "endpoint",
                    "address",
                    "site",
                    "domain",
                ];
                for alias in aliases {
                    if let Some(v) = map.get(alias).and_then(|v| v.as_str()) {
                        let trimmed = v.trim();
                        if !trimmed.is_empty() {
                            url_val = Some(trimmed.to_string());
                            break;
                        }
                    }
                }
            }

            if let Some(u) = url_val {
                let norm = crate::tools::web::normalize_web_url(&u);
                normalized["url"] = serde_json::Value::String(norm);
            }
            normalized
        }
        _ => val.clone(),
    }
}


pub fn get_server() -> &'static SearchXyzServer {
    static SERVER: OnceLock<SearchXyzServer> = OnceLock::new();
    SERVER.get_or_init(|| {
        let mut config = Config::load(None).unwrap_or_default();
        apply_openz_embedded_paths(&mut config);
        if cfg!(test) {
            let temp_dir =
                std::env::temp_dir().join(format!("searchxyz_test_index_{}", uuid::Uuid::new_v4()));
            config.index.path = temp_dir.clone();
            config.cache.path = temp_dir.join("cache.json");
        }
        let cache = Arc::new(tokio::sync::Mutex::new(Cache::load_from_file(
            config.cache.max_entries,
            config.cache.ttl_secs,
            &config.cache.path,
        )));

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.crawler.timeout_secs))
            .user_agent(&config.crawler.user_agent)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to build custom reqwest::Client for searchxyz: {e}, falling back to default");
                crate::core::http::default_http_client().clone()
            });

        let crawler = Crawler::new(
            config.crawler.clone(),
            config.headless.clone(),
            config.proxy.clone(),
            cache.clone(),
        );

        let mut backends: Vec<Box<dyn SearchBackend>> = Vec::new();
        for name in &config.search.backends {
            match name.as_str() {
                "duckduckgo" => {
                    let b = DuckDuckGoBackend::new(http_client.clone())
                        .with_proxies(crawler.clients().to_vec())
                        .with_headless(crawler.headless_browser().clone());
                    backends.push(Box::new(b));
                }
                "google" => {
                    let b = GoogleBackend::new(http_client.clone())
                        .with_proxies(crawler.clients().to_vec())
                        .with_headless(crawler.headless_browser().clone());
                    backends.push(Box::new(b));
                }
                "bing" => {
                    backends.push(Box::new(BingBackend::new(http_client.clone())));
                }
                "brave" => {
                    backends.push(Box::new(BraveBackend::new(
                        http_client.clone(),
                        config.brave.clone(),
                    )));
                }
                "searxng" => {
                    backends.push(Box::new(SearXngBackend::new(
                        http_client.clone(),
                        config.searxng.clone(),
                    )));
                }
                _ => {}
            }
        }

        let dispatcher = SearchDispatcher::new(backends);
        let extractor = ExtractionPipeline::new(config.extractor.clone());
        let index = SearchIndex::open(&config.index).unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to open searchxyz index at {:?}: {e}; falling back to temp index directory",
                config.index.path
            );
            let mut fallback_config = config.index.clone();
            let temp_dir =
                std::env::temp_dir().join(format!("searchxyz_index_{}", std::process::id()));
            let _ = std::fs::create_dir_all(&temp_dir);
            fallback_config.path = temp_dir;
            SearchIndex::open(&fallback_config).expect("failed to open fallback searchxyz index")
        });

        let graph_path = std::path::Path::new(&config.index.path).join("graph.json");
        let graph = Arc::new(tokio::sync::Mutex::new(
            KnowledgeGraph::load_from_file(&graph_path).unwrap_or_else(|_| KnowledgeGraph::new()),
        ));

        SearchXyzServer::new(dispatcher, crawler, extractor, index, cache, graph, config)
    })
}

pub fn map_mcp_err(err: rmcp::ErrorData) -> anyhow::Error {
    anyhow!("MCP Error {:?}: {}", err.code, err.message)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

