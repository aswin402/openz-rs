use std::sync::OnceLock;

pub mod core;
pub mod doctor_report;
pub mod graph;
pub mod server;
pub mod web;

pub use core::{
    config::Config,
    crawler::Crawler,
    extractor::ExtractionPipeline,
    graph::KnowledgeGraph,
    index::SearchIndex,
    search::{
        bing::BingBackend, brave::BraveBackend, duckduckgo::DuckDuckGoBackend, exa::ExaBackend,
        google::GoogleBackend, mojeek::MojeekBackend, searxng::SearXngBackend,
        tavily::TavilyBackend, websurfx::WebsurfxBackend, SearchBackend, SearchDispatcher,
    },
};
pub use graph::SearchXyzReadGithubRepoTool;
pub use server::SearchXyzServer;
pub use web::{SearchXyzBrowserSearchTool, SearchXyzDeepResearchTool};

pub(crate) use crate::tools::arguments::{
    coerce_bool_fields, coerce_numeric_fields, map_field_alias,
};

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

#[cfg(test)]
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

pub fn build_backends_for_config(
    config: &Config,
    http_client: &reqwest::Client,
    crawler: &Crawler,
) -> Vec<Box<dyn SearchBackend>> {
    let mut backends: Vec<Box<dyn SearchBackend>> = Vec::new();
    for name in &config.search.backends {
        match name.as_str() {
            "searxng" => {
                backends.push(Box::new(SearXngBackend::new(
                    http_client.clone(),
                    config.searxng.clone(),
                )));
            }
            "websurfx" => {
                backends.push(Box::new(WebsurfxBackend::new(
                    http_client.clone(),
                    config.websurfx_url.clone(),
                    config.search.timeout_secs,
                )));
            }
            "tavily" => {
                backends.push(Box::new(TavilyBackend::new(
                    http_client.clone(),
                    config.tavily_api_key.clone(),
                    config.search.timeout_secs,
                )));
            }
            "exa" => {
                backends.push(Box::new(ExaBackend::new(
                    http_client.clone(),
                    config.exa_api_key.clone(),
                    config.search.timeout_secs,
                )));
            }
            "brave" => {
                backends.push(Box::new(BraveBackend::new(
                    http_client.clone(),
                    config.brave.clone(),
                )));
            }
            "duckduckgo" => {
                let b = DuckDuckGoBackend::new(http_client.clone())
                    .with_proxies(crawler.clients().to_vec())
                    .with_headless_enabled(crawler.headless_enabled());
                backends.push(Box::new(b));
            }
            "google" => {
                let b = GoogleBackend::new(http_client.clone())
                    .with_proxies(crawler.clients().to_vec())
                    .with_headless_enabled(crawler.headless_enabled());
                backends.push(Box::new(b));
            }
            "bing" => {
                backends.push(Box::new(BingBackend::new(http_client.clone())));
            }
            "mojeek" => {
                let b = MojeekBackend::new(http_client.clone())
                    .with_proxies(crawler.clients().to_vec());
                backends.push(Box::new(b));
            }
            _ => {}
        }
    }
    backends
}

pub fn get_server() -> &'static SearchXyzServer {
    static SERVER: OnceLock<SearchXyzServer> = OnceLock::new();
    SERVER.get_or_init(|| {
        let config = Config::load(None).unwrap_or_default();

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.search.timeout_secs))
            .user_agent(&config.crawler.user_agent)
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to build custom reqwest::Client for searchxyz: {e}, falling back to default"
                );
                crate::core::http::default_http_client().clone()
            });

        let crawler = Crawler::new(
            config.crawler.clone(),
            config.headless.clone(),
            config.proxy.clone(),
        );

        let backends = build_backends_for_config(&config, &http_client, &crawler);
        let dispatcher = SearchDispatcher::new(backends);
        let extractor = ExtractionPipeline::new(config.extractor.clone());
        let index =
            SearchIndex::open(&config.index).expect("failed to initialize unified SQLite SearchIndex");

        SearchXyzServer::new(dispatcher, crawler, extractor, index, config)
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
