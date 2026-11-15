use serde::Deserialize;
use std::path::PathBuf;

use crate::config::schema::WebEngineConfig;
use crate::tools::searchxyz::core::error::SearchXyzError;

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub search: SearchConfig,
    pub brave: BraveConfig,
    pub crawler: CrawlerConfig,
    pub extractor: ExtractorConfig,
    pub index: IndexConfig,
    pub cache: CacheConfig,
    pub searxng: SearXngConfig,
    pub headless: HeadlessConfig,
    pub proxy: ProxyConfig,
    pub websurfx_url: Option<String>,
    pub tavily_api_key: Option<String>,
    pub exa_api_key: Option<String>,
    pub search_policy: String,
    pub github_max_files: usize,
    pub github_max_bytes: u64,
    pub github_clone_timeout_secs: u64,
    pub auto_read_top_results_limit: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ServerConfig {
    pub name: String,
    pub version: String,
    pub log_level: String,
    pub auth_token: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct SearchConfig {
    pub backends: Vec<String>,
    pub max_results: usize,
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct BraveConfig {
    pub api_key: Option<String>,
    pub max_results: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct CrawlerConfig {
    pub timeout_secs: u64,
    pub user_agent: String,
    pub max_body_bytes: usize,
    pub max_redirects: usize,
    pub max_retries: u32,
    pub rate_limit_per_sec: u64,
    pub allow_private_network: bool,
    pub default_cache_ttl_secs: u64,
    pub max_concurrent_fetches: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ExtractorConfig {
    pub min_content_length: usize,
    pub strip_selectors: Vec<String>,
    pub content_selectors: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct EmbeddingConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_url: Option<String>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "local".to_string(),
            model: "bge-small-en-v1.5".to_string(),
            api_key: None,
            api_url: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct IndexConfig {
    pub path: PathBuf,
    pub writer_heap_bytes: usize,
    pub embedding: EmbeddingConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub ttl_secs: u64,
    pub path: PathBuf,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "searchxyz".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            log_level: "info".into(),
            auth_token: None,
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            backends: vec![
                "duckduckgo".into(),
                "google".into(),
                "bing".into(),
                "mojeek".into(),
                "brave".into(),
            ],
            max_results: 10,
            timeout_secs: 12,
        }
    }
}

impl Default for BraveConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            max_results: 10,
        }
    }
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            user_agent: "OpenZ-SearchXyz/1.0 (+https://github.com/aswin402/openz-rs)".into(),
            max_body_bytes: 5 * 1024 * 1024,
            max_redirects: 5,
            max_retries: 2,
            rate_limit_per_sec: 4,
            allow_private_network: false,
            default_cache_ttl_secs: 3600,
            max_concurrent_fetches: 8,
        }
    }
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        Self {
            min_content_length: 50,
            strip_selectors: vec![
                "script".into(),
                "style".into(),
                "nav".into(),
                "footer".into(),
                "header".into(),
                "aside".into(),
                "noscript".into(),
                "iframe".into(),
            ],
            content_selectors: vec![
                "article".into(),
                "main".into(),
                "[role=\"main\"]".into(),
                ".post-content".into(),
                ".article-body".into(),
            ],
        }
    }
}

fn default_openz_base() -> PathBuf {
    match std::env::var("OPENZ_CONFIG_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".openz"),
    }
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            path: default_openz_base().join("memory.db"),
            writer_heap_bytes: 0,
            embedding: EmbeddingConfig::default(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 2_000,
            ttl_secs: 3600,
            path: default_openz_base().join("memory.db"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct SearXngConfig {
    pub instance_url: String,
    pub engines: Option<String>,
    pub timeout_secs: u64,
}

impl Default for SearXngConfig {
    fn default() -> Self {
        Self {
            instance_url: String::new(),
            engines: None,
            timeout_secs: 8,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct HeadlessConfig {
    pub enabled: bool,
    pub chrome_path: Option<String>,
    pub wait_after_load_ms: u64,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            chrome_path: None,
            wait_after_load_ms: 1000,
            viewport_width: 1280,
            viewport_height: 800,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub urls: Vec<String>,
}

pub(crate) fn parse_backend_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|backend| !backend.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn non_empty(opt: Option<String>) -> Option<String> {
    opt.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn env_non_empty(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
}

fn env_parsed<T: std::str::FromStr>(keys: &[&str]) -> Option<T> {
    env_non_empty(keys).and_then(|val| val.parse().ok())
}

fn toml_has_key_path(contents: &str, path: &[&str]) -> Result<bool, SearchXyzError> {
    let value = contents.parse::<toml::Value>().map_err(|e| {
        SearchXyzError::ConfigError(format!("Failed to parse TOML config metadata: {e}"))
    })?;
    let mut current = &value;
    for key in path {
        match current.get(*key) {
            Some(next) => current = next,
            None => return Ok(false),
        }
    }
    Ok(true)
}

fn parse_toml_config(
    contents: &str,
    label: &str,
) -> Result<(Config, bool, bool), SearchXyzError> {
    let explicit_backends = toml_has_key_path(contents, &["search", "backends"])?;
    let explicit_searxng = toml_has_key_path(contents, &["searxng", "instance_url"])?;
    let config = toml::from_str::<Config>(contents)
        .map_err(|e| SearchXyzError::ConfigError(format!("Failed to parse {label}: {e}")))?;
    Ok((config, explicit_backends, explicit_searxng))
}

impl Config {
    #[allow(clippy::field_reassign_with_default)]
    pub fn from_web_engine_config(web: &WebEngineConfig) -> Self {
        let mut cfg = Self::default();
        cfg.search_policy = web.search_policy.clone();
        cfg.searxng.instance_url = non_empty(web.searxng_url.clone()).unwrap_or_default();
        cfg.searxng.timeout_secs = web.search_timeout_secs.max(1);
        cfg.websurfx_url = non_empty(web.websurfx_url.clone());
        cfg.brave.api_key = non_empty(web.brave_api_key.clone());
        cfg.tavily_api_key = non_empty(web.tavily_api_key.clone());
        cfg.exa_api_key = non_empty(web.exa_api_key.clone());
        cfg.search.timeout_secs = web.search_timeout_secs.max(1);
        cfg.crawler.user_agent = match web.user_agent.trim().is_empty() {
            true => cfg.crawler.user_agent,
            false => web.user_agent.clone(),
        };
        cfg.crawler.timeout_secs = web.fetch_timeout_secs.max(1);
        cfg.crawler.max_body_bytes = web.max_content_bytes.max(1024);
        cfg.crawler.default_cache_ttl_secs = web.cache_ttl_secs;
        cfg.crawler.max_concurrent_fetches = web.max_concurrent_fetches.clamp(1, 32);
        cfg.crawler.allow_private_network = web.allow_private_network;
        cfg.cache.ttl_secs = web.cache_ttl_secs;
        cfg.github_max_files = web.github_max_files.clamp(1, 10_000);
        cfg.github_max_bytes = (web.github_max_bytes as u64).clamp(1024, 200 * 1024 * 1024);
        cfg.github_clone_timeout_secs = web.github_clone_timeout_secs.clamp(5, 600);
        cfg.auto_read_top_results_limit = web.auto_read_top_results_limit.clamp(1, 5);
        cfg
    }

    pub fn load(path: Option<&str>) -> Result<Self, SearchXyzError> {
        let (mut config, file_has_explicit_backends, file_has_explicit_searxng) = match path {
            Some(p) => {
                let contents = std::fs::read_to_string(p)?;
                parse_toml_config(&contents, p)?
            }
            None => {
                let web = crate::config::loader::load_config()
                    .map(|c| c.web)
                    .unwrap_or_default();
                let explicit_searxng = web
                    .searxng_url
                    .as_deref()
                    .is_some_and(|u| !u.trim().is_empty());
                (Self::from_web_engine_config(&web), false, explicit_searxng)
            }
        };

        let env_has_explicit_backends =
            env_non_empty(&["OPENZ_WEB_SEARCH_BACKENDS", "SEARCHXYZ_SEARCH_BACKENDS"]).is_some();
        let env_has_explicit_searxng =
            env_non_empty(&["OPENZ_SEARXNG_URL", "SEARCHXYZ_SEARXNG_URL", "SEARXNG_URL"]).is_some();

        config.apply_env_overrides();

        let explicit_backend_order = file_has_explicit_backends || env_has_explicit_backends;
        let explicit_searxng = file_has_explicit_searxng || env_has_explicit_searxng;
        config.apply_native_backend_preferences(explicit_backend_order, explicit_searxng);
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn apply_env_overrides(&mut self) {
        if_some_set(env_non_empty(&["SEARCHXYZ_API_KEY"]), |key| {
            self.server.auth_token = Some(key);
        });
        if_some_set(
            env_non_empty(&["OPENZ_WEB_SEARCH_POLICY"]),
            |policy| self.search_policy = policy,
        );
        if_some_set(
            env_non_empty(&["OPENZ_BRAVE_API_KEY", "SEARCHXYZ_BRAVE_API_KEY", "BRAVE_API_KEY"]),
            |key| self.brave.api_key = Some(key),
        );
        if_some_set(
            env_non_empty(&["OPENZ_TAVILY_API_KEY", "SEARCHXYZ_TAVILY_API_KEY", "TAVILY_API_KEY"]),
            |key| self.tavily_api_key = Some(key),
        );
        if_some_set(
            env_non_empty(&["OPENZ_EXA_API_KEY", "SEARCHXYZ_EXA_API_KEY", "EXA_API_KEY"]),
            |key| self.exa_api_key = Some(key),
        );
        if_some_set(
            env_non_empty(&["OPENZ_WEBSURFX_URL", "SEARCHXYZ_WEBSURFX_URL", "WEBSURFX_URL"]),
            |url| self.websurfx_url = Some(url),
        );
        if_some_set(
            env_non_empty(&["OPENZ_SEARXNG_URL", "SEARCHXYZ_SEARXNG_URL", "SEARXNG_URL"]),
            |url| self.searxng.instance_url = url,
        );
        if_some_set(
            env_non_empty(&["OPENZ_WEB_SEARCH_BACKENDS", "SEARCHXYZ_SEARCH_BACKENDS"]),
            |backends| self.search.backends = parse_backend_list(&backends),
        );
        if_some_set(env_non_empty(&["SEARCHXYZ_LOG_LEVEL"]), |level| {
            self.server.log_level = level;
        });
        if_some_set(env_non_empty(&["SEARCHXYZ_INDEX_PATH"]), |path| {
            self.index.path = PathBuf::from(path);
        });
        if_some_set(env_non_empty(&["SEARCHXYZ_EMBEDDING_PROVIDER"]), |prov| {
            self.index.embedding.provider = prov;
        });
        if_some_set(env_non_empty(&["SEARCHXYZ_EMBEDDING_MODEL"]), |model| {
            self.index.embedding.model = model;
        });
        if_some_set(env_non_empty(&["SEARCHXYZ_EMBEDDING_API_KEY"]), |key| {
            self.index.embedding.api_key = Some(key);
        });
        if_some_set(env_non_empty(&["SEARCHXYZ_EMBEDDING_URL"]), |url| {
            self.index.embedding.api_url = Some(url);
        });

        let provider_lc = self.index.embedding.provider.to_lowercase();
        for (env_var, target_provider) in [
            ("SEARCHXYZ_OPENAI_API_KEY", "openai"),
            ("SEARCHXYZ_GEMINI_API_KEY", "gemini"),
            ("SEARCHXYZ_COHERE_API_KEY", "cohere"),
        ] {
            match (std::env::var(env_var), provider_lc == target_provider) {
                (Ok(key), true) => self.index.embedding.api_key = Some(key),
                _ => {}
            }
        }

        if_some_set(env_parsed(&["SEARCHXYZ_CACHE_MAX_ENTRIES"]), |n| {
            self.cache.max_entries = n;
        });
        if_some_set(
            env_parsed(&["OPENZ_WEB_CACHE_TTL_SECS", "SEARCHXYZ_CACHE_TTL_SECS"]),
            |n| {
                self.cache.ttl_secs = n;
                self.crawler.default_cache_ttl_secs = n;
            },
        );
        if_some_set(env_non_empty(&["SEARCHXYZ_CACHE_PATH"]), |path| {
            self.cache.path = PathBuf::from(path);
        });
        if_some_set(env_parsed(&["SEARCHXYZ_HEADLESS_ENABLED"]), |b| {
            self.headless.enabled = b;
        });
        if_some_set(env_non_empty(&["SEARCHXYZ_CHROME_PATH"]), |path| {
            self.headless.chrome_path = Some(path);
        });
        if_some_set(env_parsed(&["SEARCHXYZ_HEADLESS_WAIT_MS"]), |n| {
            self.headless.wait_after_load_ms = n;
        });
        if_some_set(env_parsed(&["SEARCHXYZ_PROXY_ENABLED"]), |b| {
            self.proxy.enabled = b;
        });
        if_some_set(env_non_empty(&["SEARCHXYZ_PROXY_URLS"]), |urls_str| {
            self.proxy.urls = urls_str
                .split(',')
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .map(ToString::to_string)
                .collect();
        });
        if_some_set(
            env_parsed(&["OPENZ_WEB_ALLOW_PRIVATE_NETWORK", "SEARCHXYZ_ALLOW_PRIVATE_NETWORK"]),
            |allow| self.crawler.allow_private_network = allow,
        );
    }

    fn apply_native_backend_preferences(
        &mut self,
        explicit_backend_order: bool,
        explicit_searxng: bool,
    ) {
        match explicit_backend_order {
            true => return,
            false => {}
        }

        let priority_candidates = [
            (
                "searxng",
                explicit_searxng && !self.searxng.instance_url.trim().is_empty(),
            ),
            ("websurfx", self.websurfx_url.is_some()),
            ("tavily", self.tavily_api_key.is_some()),
            ("exa", self.exa_api_key.is_some()),
            ("brave", self.brave.api_key.is_some()),
        ];

        let mut preferred: Vec<String> = priority_candidates
            .into_iter()
            .filter(|(_, active)| *active)
            .map(|(name, _)| name.to_string())
            .collect();

        for existing in &self.search.backends {
            match preferred.iter().any(|item| item == existing) {
                true => {}
                false => preferred.push(existing.clone()),
            }
        }
        self.search.backends = preferred;
    }

    fn validate(&self) -> Result<(), SearchXyzError> {
        match self.search.backends.is_empty() {
            true => {
                return Err(SearchXyzError::ConfigError(
                    "At least one search backend must be configured".into(),
                ));
            }
            false => {}
        }
        match self.crawler.max_body_bytes == 0 {
            true => Err(SearchXyzError::ConfigError(
                "crawler.max_body_bytes must be > 0".into(),
            )),
            false => Ok(()),
        }
    }
}

fn if_some_set<T, F: FnOnce(T)>(opt: Option<T>, f: F) {
    match opt {
        Some(val) => f(val),
        None => {}
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert!(!config.proxy.enabled);
        assert!(config.proxy.urls.is_empty());
        assert!(config.index.path.ends_with("memory.db"));
    }

    #[test]
    fn test_env_overrides() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEARCHXYZ_PROXY_ENABLED", "true");
        std::env::set_var(
            "SEARCHXYZ_PROXY_URLS",
            "http://proxy1:8080, socks5://proxy2:1080",
        );
        std::env::set_var("SEARCHXYZ_CACHE_PATH", "/tmp/searchxyz-test-cache.json");

        let mut config = Config::default();
        config.apply_env_overrides();

        assert!(config.proxy.enabled);
        assert_eq!(
            config.proxy.urls,
            vec![
                "http://proxy1:8080".to_string(),
                "socks5://proxy2:1080".to_string()
            ]
        );
        assert_eq!(
            config.cache.path,
            PathBuf::from("/tmp/searchxyz-test-cache.json")
        );

        std::env::remove_var("SEARCHXYZ_PROXY_ENABLED");
        std::env::remove_var("SEARCHXYZ_PROXY_URLS");
        std::env::remove_var("SEARCHXYZ_CACHE_PATH");
    }

    #[test]
    fn searxng_env_is_preferred_when_backends_are_not_explicit() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEARCHXYZ_SEARXNG_URL", "http://searxng.local");
        std::env::remove_var("SEARCHXYZ_SEARCH_BACKENDS");
        std::env::remove_var("OPENZ_WEB_SEARCH_BACKENDS");

        let config = Config::load(None).unwrap();

        assert_eq!(
            config.search.backends.first().map(String::as_str),
            Some("searxng")
        );
        assert_eq!(config.searxng.instance_url, "http://searxng.local");

        std::env::remove_var("SEARCHXYZ_SEARXNG_URL");
    }

    #[test]
    fn explicit_search_backends_keep_user_order_even_with_searxng_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEARCHXYZ_SEARXNG_URL", "http://searxng.local");
        std::env::set_var("SEARCHXYZ_SEARCH_BACKENDS", "duckduckgo,searxng,brave");

        let config = Config::load(None).unwrap();

        assert_eq!(
            config.search.backends,
            vec![
                "duckduckgo".to_string(),
                "searxng".to_string(),
                "brave".to_string()
            ]
        );

        std::env::remove_var("SEARCHXYZ_SEARXNG_URL");
        std::env::remove_var("SEARCHXYZ_SEARCH_BACKENDS");
    }

    #[test]
    fn parse_backend_list_ignores_empty_items() {
        assert_eq!(
            parse_backend_list("duckduckgo, ,searxng,,brave"),
            vec![
                "duckduckgo".to_string(),
                "searxng".to_string(),
                "brave".to_string()
            ]
        );
    }

    #[test]
    fn test_embedding_config_loading_and_overrides() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEARCHXYZ_EMBEDDING_PROVIDER", "openai");
        std::env::set_var("SEARCHXYZ_EMBEDDING_MODEL", "custom-model");
        std::env::set_var("SEARCHXYZ_EMBEDDING_API_KEY", "custom-key");
        std::env::set_var("SEARCHXYZ_EMBEDDING_URL", "http://custom-url/v1");
        std::env::set_var("SEARCHXYZ_OPENAI_API_KEY", "openai-override-key");

        let mut config = Config::default();
        config.apply_env_overrides();

        assert_eq!(config.index.embedding.provider, "openai");
        assert_eq!(config.index.embedding.model, "custom-model");
        assert_eq!(
            config.index.embedding.api_key,
            Some("openai-override-key".to_string())
        );
        assert_eq!(
            config.index.embedding.api_url,
            Some("http://custom-url/v1".to_string())
        );

        std::env::remove_var("SEARCHXYZ_EMBEDDING_PROVIDER");
        std::env::remove_var("SEARCHXYZ_EMBEDDING_MODEL");
        std::env::remove_var("SEARCHXYZ_EMBEDDING_API_KEY");
        std::env::remove_var("SEARCHXYZ_EMBEDDING_URL");
        std::env::remove_var("SEARCHXYZ_OPENAI_API_KEY");
    }

    #[test]
    fn test_auth_token_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEARCHXYZ_API_KEY", "my-secret-token");

        let mut config = Config::default();
        config.apply_env_overrides();

        assert_eq!(
            config.server.auth_token,
            Some("my-secret-token".to_string())
        );

        std::env::remove_var("SEARCHXYZ_API_KEY");
    }
}
