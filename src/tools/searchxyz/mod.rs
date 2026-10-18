use anyhow::anyhow;
use std::sync::{Arc, OnceLock};

pub mod core;
pub mod graph;
pub mod index;
pub mod server;
pub mod web;

pub use core::{
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
};
pub use server::SearchXyzServer;

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
    match std::env::var("OPENZ_CONFIG_DIR") {
        Ok(path) => std::path::PathBuf::from(path),
        Err(_) => dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".openz"),
    }
}

fn apply_openz_embedded_paths(config: &mut Config) {
    let defaults = Config::default();
    let base = openz_config_dir().join("searchxyz");

    match std::env::var_os("SEARCHXYZ_INDEX_PATH").is_none()
        && config.index.path == defaults.index.path
    {
        true => config.index.path = base.join("index"),
        false => {}
    }

    match std::env::var_os("SEARCHXYZ_CACHE_PATH").is_none()
        && config.cache.path == defaults.cache.path
    {
        true => config.cache.path = base.join("cache.json"),
        false => {}
    }
}

pub(crate) fn coerce_number_value(val: &serde_json::Value) -> Option<serde_json::Value> {
    match val {
        serde_json::Value::Number(_) => Some(val.clone()),
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            trimmed
                .parse::<i64>()
                .ok()
                .map(|i| serde_json::Value::Number(serde_json::Number::from(i)))
                .or_else(|| {
                    trimmed
                        .parse::<u64>()
                        .ok()
                        .map(|u| serde_json::Value::Number(serde_json::Number::from(u)))
                })
                .or_else(|| {
                    trimmed
                        .parse::<f64>()
                        .ok()
                        .and_then(serde_json::Number::from_f64)
                        .map(serde_json::Value::Number)
                })
        }
        _ => None,
    }
}

pub(crate) fn coerce_numeric_fields(val: &mut serde_json::Value, field_names: &[&str]) {
    let map = match val.as_object_mut() {
        Some(m) => m,
        None => return,
    };
    for &name in field_names {
        let coerced = map.get(name).and_then(coerce_number_value);
        match coerced {
            Some(c) => {
                map.insert(name.to_string(), c);
            }
            None => {}
        }
    }
}

fn coerce_single_bool(entry: &serde_json::Value) -> Option<bool> {
    match entry {
        serde_json::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(1) => Some(true),
            Some(0) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn coerce_bool_fields(val: &mut serde_json::Value, field_names: &[&str]) {
    let map = match val.as_object_mut() {
        Some(m) => m,
        None => return,
    };
    for &name in field_names {
        let coerced = map.get(name).and_then(coerce_single_bool);
        match coerced {
            Some(b) => {
                map.insert(name.to_string(), serde_json::Value::Bool(b));
            }
            None => {}
        }
    }
}

pub(crate) fn map_field_alias(val: &mut serde_json::Value, canonical: &str, aliases: &[&str]) {
    let map = match val.as_object_mut() {
        Some(m) => m,
        None => return,
    };
    let has_canonical = map.get(canonical).is_some_and(|v| !v.is_null());
    match has_canonical {
        true => {}
        false => {
            let found = aliases
                .iter()
                .find_map(|&alias| map.get(alias).filter(|v| !v.is_null()).cloned());
            match found {
                Some(cloned) => {
                    map.insert(canonical.to_string(), cloned);
                }
                None => {}
            }
        }
    }
}

pub(crate) fn wrap_string_arg(val: &serde_json::Value, key: &str) -> serde_json::Value {
    match val.as_str() {
        Some(s) => serde_json::json!({ key: s.trim() }),
        None => val.clone(),
    }
}

pub(crate) fn wrap_url_string_arg(val: &serde_json::Value, key: &str) -> serde_json::Value {
    match val.as_str() {
        Some(s) => serde_json::json!({ key: crate::tools::web::normalize_web_url(s) }),
        None => val.clone(),
    }
}

pub(crate) fn normalize_url_field(val: &mut serde_json::Value, key: &str) {
    let normalized_url = val
        .get(key)
        .and_then(|v| v.as_str())
        .map(crate::tools::web::normalize_web_url);
    match normalized_url {
        Some(u) => val[key] = serde_json::Value::String(u),
        None => {}
    }
}

pub(crate) fn default_entity_type_from_field(
    val: &mut serde_json::Value,
    type_field: &str,
    name_field: &str,
) {
    match val.get(type_field).is_none() {
        false => {}
        true => {
            let is_url = val
                .get(name_field)
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.contains("://"));
            let label = match is_url {
                true => "Document",
                false => "Entity",
            };
            val[type_field] = serde_json::json!(label);
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
                .is_some_and(|s| !s.trim().is_empty());

            match has_valid_query {
                true => {}
                false => {
                    let aliases = ["q", "search", "prompt", "term", "keywords", "text"];
                    let found = aliases.into_iter().find_map(|alias| {
                        map.get(alias)
                            .and_then(|v| v.as_str())
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                    });
                    match found {
                        Some(q) => normalized["query"] = serde_json::Value::String(q),
                        None => {}
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
            let aliases = [
                "url",
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
            let url_val = aliases.into_iter().find_map(|alias| {
                map.get(alias)
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            });

            match url_val {
                Some(u) => {
                    let norm = crate::tools::web::normalize_web_url(&u);
                    normalized["url"] = serde_json::Value::String(norm);
                }
                None => {}
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
        match cfg!(test) {
            true => {
                let temp_dir = std::env::temp_dir()
                    .join(format!("searchxyz_test_index_{}", uuid::Uuid::new_v4()));
                config.index.path = temp_dir.clone();
                config.cache.path = temp_dir.join("cache.json");
            }
            false => {}
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

