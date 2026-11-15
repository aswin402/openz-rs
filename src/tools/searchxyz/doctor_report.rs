use anyhow::Result;
use serde_json::{json, Value};

use super::core::config::Config;
use super::core::search::cooldown::active_search_backend_cooldowns;
use super::get_server;

pub fn backend_health_label(
    name: &str,
    configured_order: &[String],
    config: &Config,
) -> String {
    let enabled = configured_order.iter().any(|backend| backend == name);
    let preferred = configured_order
        .first()
        .is_some_and(|backend| backend == name);
    match enabled {
        false => return "disabled".to_string(),
        true => {}
    }

    let base = match name {
        "searxng" => match config.searxng.instance_url.trim().is_empty() {
            true => "missing_config",
            false => "configured",
        },
        "websurfx" => match config
            .websurfx_url
            .as_deref()
            .is_some_and(|u| !u.trim().is_empty())
        {
            true => "configured",
            false => "missing_config",
        },
        "brave" => match config
            .brave
            .api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
        {
            true => "configured",
            false => "missing_key",
        },
        "tavily" => match config
            .tavily_api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
        {
            true => "configured",
            false => "missing_key",
        },
        "exa" => match config
            .exa_api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
        {
            true => "configured",
            false => "missing_key",
        },
        "duckduckgo" | "google" | "bing" | "mojeek" => "keyless",
        _ => "configured",
    };

    match preferred {
        true => format!("preferred ({base})"),
        false => base.to_string(),
    }
}

pub fn only_scraper_backends_enabled(configured_order: &[String]) -> bool {
    !configured_order.is_empty()
        && configured_order.iter().all(|backend| {
            matches!(
                backend.as_str(),
                "duckduckgo" | "google" | "bing" | "mojeek"
            )
        })
}

pub fn only_scraper_backends_usable(configured_order: &[String], config: &Config) -> bool {
    let usable: Vec<&str> = configured_order
        .iter()
        .map(String::as_str)
        .filter(|name| {
            let label = backend_health_label(name, configured_order, config);
            !label.contains("missing_key")
                && !label.contains("missing_config")
                && label != "disabled"
        })
        .collect();
    !usable.is_empty()
        && usable
            .into_iter()
            .all(|name| matches!(name, "duckduckgo" | "google" | "bing" | "mojeek"))
}

fn active_policy_string(config: &Config) -> String {
    std::env::var("OPENZ_WEB_SEARCH_POLICY")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| match config.search_policy.trim().is_empty() {
            true => "native_only".to_string(),
            false => config.search_policy.clone(),
        })
}

pub fn build_web_engine_health_json() -> Value {
    let server = get_server();
    let config = server.config.as_ref();
    let known_backends = [
        "searxng",
        "websurfx",
        "tavily",
        "exa",
        "brave",
        "duckduckgo",
        "google",
        "bing",
        "mojeek",
    ];
    let mut backend_health = serde_json::Map::new();
    for backend in known_backends {
        backend_health.insert(
            backend.to_string(),
            Value::String(backend_health_label(
                backend,
                &config.search.backends,
                config,
            )),
        );
    }

    let cache_stats = crate::tools::shared_memory::web_cache::http_cache_stats()
        .map(|stats| {
            json!({
                "total_entries": stats.entry_count,
                "fresh_entries": stats.fresh_count,
                "total_bytes": stats.total_body_bytes,
            })
        })
        .unwrap_or_else(|_| json!({ "total_entries": 0, "fresh_entries": 0, "total_bytes": 0 }));

    let research_stats = crate::tools::shared_memory::research_bundle::research_index_stats()
        .map(|stats| {
            json!({
                "total_documents": stats.chunk_count,
                "distinct_sources": stats.distinct_sources,
                "distinct_urls": stats.distinct_urls,
                "fts_rows": stats.fts_rows,
            })
        })
        .unwrap_or_else(|_| {
            json!({
                "total_documents": 0,
                "distinct_sources": 0,
                "distinct_urls": 0,
                "fts_rows": 0,
            })
        });

    json!({
        "engine": "searchxyz_native",
        "storage": "sqlite_unified_memory_db",
        "search_policy": active_policy_string(config),
        "configured_backends": config.search.backends,
        "backend_health": backend_health,
        "only_scraper_backends": only_scraper_backends_usable(&config.search.backends, config),
        "headless_enabled": config.headless.enabled,
        "allow_private_network": config.crawler.allow_private_network,
        "max_body_bytes": config.crawler.max_body_bytes,
        "http_cache": cache_stats,
        "research_index": research_stats,
        "active_cooldowns": active_search_backend_cooldowns(),
    })
}

pub async fn build_searchxyz_doctor_report(include_paths: bool) -> Result<String> {
    let server = get_server();
    let config = server.config.as_ref();
    let cache_stats =
        crate::tools::shared_memory::web_cache::http_cache_stats().unwrap_or_default();
    let (_, source_count) = server.index.list_documents(None, 1, 0)?;

    let native_policy = active_policy_string(config);
    let known_backends = [
        "searxng",
        "websurfx",
        "tavily",
        "exa",
        "brave",
        "duckduckgo",
        "google",
        "bing",
        "mojeek",
    ];
    let headless_status = match (config.headless.enabled, &config.headless.chrome_path) {
        (true, Some(path)) => format!("enabled ({path})"),
        (true, None) => "enabled (OpenZ BrowserBroker)".to_string(),
        (false, _) => "disabled".to_string(),
    };

    let mut report = String::new();
    report.push_str("# SearchXyz Doctor\n\n");
    report.push_str("## Policy\n");
    report.push_str(&format!("- OpenZ web_search policy: `{}`\n", native_policy));
    report.push_str("- Default behavior: native SearchXyz engine unified with SQLite memory.db and BrowserBroker.\n\n");

    report.push_str("## Search Backends\n");
    report.push_str(&format!(
        "- Configured order: `{}`\n",
        config.search.backends.join(", ")
    ));
    report.push_str(&format!("- Max results: {}\n", config.search.max_results));
    report.push_str("- Ranking: enabled (RRF + dynamic term rarity, coverage, phrase/entity matches, domain bonuses)\n");
    for backend in known_backends {
        report.push_str(&format!(
            "- {}: {}\n",
            backend,
            backend_health_label(backend, &config.search.backends, config)
        ));
    }
    report.push_str("- Labels: preferred, configured, missing_key, missing_config, disabled, keyless.\n\n");

    report.push_str("## Fetch & Safety\n");
    report.push_str(&format!("- Headless rendering: {}\n", headless_status));
    report.push_str(&format!(
        "- Private network fetches allowed: {}\n",
        config.crawler.allow_private_network
    ));
    report.push_str(&format!(
        "- Request timeout: {}s\n",
        config.crawler.timeout_secs
    ));
    report.push_str(&format!(
        "- Max redirects: {}\n",
        config.crawler.max_redirects
    ));
    report.push_str(&format!(
        "- Max body bytes: {}\n\n",
        config.crawler.max_body_bytes
    ));

    report.push_str("## Local State\n");
    report.push_str(&format!(
        "- SQLite HTTP cache entries: {} ({} fresh)\n",
        cache_stats.entry_count, cache_stats.fresh_count
    ));
    report.push_str(&format!("- Indexed research sources: {}\n", source_count));
    match include_paths {
        true => {
            report.push_str(&format!(
                "- Unified memory DB path: `{}`\n",
                config.index.path.display()
            ));
        }
        false => {}
    }

    let active_cooldowns = active_search_backend_cooldowns();
    match active_cooldowns.is_empty() {
        true => {}
        false => {
            report.push_str("\n## Backend Cooldowns\n");
            for cooldown in active_cooldowns {
                let backend = cooldown["backend"].as_str().unwrap_or("unknown");
                let error_kind = cooldown["error_kind"].as_str().unwrap_or("unknown");
                let remaining = cooldown["cooldown_remaining_secs"].as_u64().unwrap_or(0);
                report.push_str(&format!(
                    "- {}: {} ({}s remaining)\n",
                    backend, error_kind, remaining
                ));
            }
        }
    }

    report.push_str("\n## Hints\n");
    match config
        .search
        .backends
        .iter()
        .any(|backend| backend == "searxng")
    {
        false => {
            report.push_str("- SearXNG is not enabled in backend order. Configure `web.searxng_url` in `~/.openz/config.json` or `OPENZ_SEARXNG_URL` for private self-hosted metasearch.\n");
        }
        true => {}
    }
    match only_scraper_backends_enabled(&config.search.backends) {
        true => {
            report.push_str("- Only keyless scraper backends are enabled; configure SearXNG, Websurfx, Brave, Tavily, or Exa when under anti-bot pressure.\n");
        }
        false => {}
    }
    report.push_str("- Provider-free browser fallback is automatic in default `web_search`; call `searchxyz_browser_search` directly only for explicit engine/debug control.\n");

    Ok(report)
}
