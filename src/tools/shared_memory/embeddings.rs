use super::db::get_shared_client;
use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Managed embedding model with idle eviction.
/// The ONNX model (~130 MB) is loaded on first use and automatically
/// evicted after 5 minutes of inactivity to free memory.
struct ManagedModel {
    inner: std::sync::Mutex<Option<(TextEmbedding, Instant)>>,
    eviction_started: std::sync::atomic::AtomicBool,
}

static MANAGED_MODEL: OnceLock<ManagedModel> = OnceLock::new();

const MODEL_IDLE_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

fn get_managed() -> &'static ManagedModel {
    MANAGED_MODEL.get_or_init(|| ManagedModel {
        inner: std::sync::Mutex::new(None),
        eviction_started: std::sync::atomic::AtomicBool::new(false),
    })
}

/// Start the background eviction task (idempotent — only spawns once).
pub fn start_model_eviction() {
    let mgr = get_managed();
    match mgr.eviction_started.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::Relaxed,
    ) {
        Ok(_) => {
            tokio::spawn(async {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    let mgr = get_managed();
                    let mut guard = mgr.inner.lock().unwrap_or_else(|p| p.into_inner());
                    let should_evict = guard
                        .as_ref()
                        .is_some_and(|(_, last_used)| last_used.elapsed() > MODEL_IDLE_TIMEOUT);
                    match should_evict {
                        true => {
                            *guard = None;
                            tracing::info!(
                                "Evicted FastEmbed ONNX model after {}s idle — freed ~130 MB",
                                MODEL_IDLE_TIMEOUT.as_secs()
                            );
                        }
                        false => {}
                    }
                }
            });
        }
        Err(_) => {}
    }
}

/// Load the embedding model (if not already loaded), reset the idle timer,
/// and run the provided closure with a mutable reference to the model.
pub fn with_model<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut TextEmbedding) -> Result<R>,
{
    let mgr = get_managed();
    let mut guard = mgr
        .inner
        .lock()
        .map_err(|e| anyhow!("Managed model lock poisoned: {}", e))?;

    match guard.is_none() {
        true => {
            let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))?;
            *guard = Some((model, Instant::now()));
            tracing::info!("Loaded FastEmbed ONNX model (AllMiniLmL6V2)");
        }
        false => {}
    }

    match *guard {
        Some((_, ref mut ts)) => *ts = Instant::now(),
        None => {}
    }

    let (ref mut model, _) = guard
        .as_mut()
        .ok_or_else(|| anyhow!("Model unexpectedly None after init"))?;
    f(model)
}

pub(crate) fn resolve_cohere_embed_url(
    cohere_config: &crate::config::schema::ProviderConfig,
) -> String {
    cohere_config
        .api_base
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|trimmed| {
            let base_clean = trimmed.trim_end_matches('/');
            match (
                base_clean.ends_with("/embed"),
                base_clean.ends_with("/v1"),
            ) {
                (true, _) => base_clean.to_string(),
                (false, true) => format!("{base_clean}/embed"),
                (false, false) => format!("{base_clean}/v1/embed"),
            }
        })
        .unwrap_or_else(|| "https://api.cohere.com/v1/embed".to_string())
}

fn ordered_embedding_providers(config: &crate::config::schema::Config) -> Vec<&'static str> {
    let preferred = config
        .embeddings
        .as_ref()
        .and_then(|e| e.preferred_provider.as_deref());
    let mut providers_order = vec!["google", "cohere", "openai"];
    match preferred {
        Some(pref) => {
            let p_clean = pref.to_lowercase();
            let pos_opt = providers_order.iter().position(|&p| {
                p == p_clean
                    || (p == "openai" && (p_clean == "opencode_zen" || p_clean == "opencode-zen"))
            });
            match pos_opt {
                Some(pos) => {
                    let removed = providers_order.remove(pos);
                    providers_order.insert(0, removed);
                }
                None => {}
            }
        }
        None => {}
    }
    providers_order
}

fn resolve_provider_key(cfg: &crate::config::schema::ProviderConfig) -> Option<&str> {
    cfg.api_key
        .as_deref()
        .or_else(|| cfg.extra.get("apiKey").and_then(|v| v.as_str()))
        .filter(|k| !k.trim().is_empty())
}

fn resolve_openai_embed_credentials(
    config: &crate::config::schema::Config,
) -> Option<(String, String)> {
    [
        config.providers.opencode_zen.as_ref(),
        config.providers.openai.as_ref(),
    ]
    .into_iter()
    .flatten()
    .find_map(|cfg| {
        let key = resolve_provider_key(cfg)?.to_string();
        let base = cfg
            .api_base
            .as_deref()
            .or_else(|| cfg.extra.get("apiBase").and_then(|v| v.as_str()))
            .unwrap_or("https://api.openai.com/v1")
            .to_string();
        Some((key, base))
    })
}

fn parse_f32_array(arr: &[serde_json::Value]) -> Vec<f32> {
    arr.iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect()
}

async fn post_json_value(req: reqwest::RequestBuilder) -> Option<serde_json::Value> {
    let res = req.send().await.ok()?;
    match res.status().is_success() {
        true => res.json::<serde_json::Value>().await.ok(),
        false => None,
    }
}

async fn get_cloud_embedding(text: &str, is_query: bool) -> Result<Vec<f32>> {
    let config = crate::config::loader::load_config()?;
    let providers_order = ordered_embedding_providers(&config);

    for provider_name in providers_order {
        match provider_name {
            "google" => {
                let key_opt = config
                    .providers
                    .google_ai_studio
                    .as_ref()
                    .and_then(resolve_provider_key);
                match key_opt {
                    Some(key) => {
                        let url = format!(
                            "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key={}",
                            key
                        );
                        let req = get_shared_client().post(&url).json(&serde_json::json!({
                            "content": {
                                "parts": [{
                                    "text": text
                                }]
                            }
                        }));
                        let vec = post_json_value(req)
                            .await
                            .and_then(|json| {
                                json.pointer("/embedding/values")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| parse_f32_array(arr))
                            })
                            .filter(|v| !v.is_empty());
                        match vec {
                            Some(v) => return Ok(v),
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            "cohere" => {
                let cohere_opt = config
                    .providers
                    .cohere
                    .as_ref()
                    .and_then(|cfg| resolve_provider_key(cfg).map(|k| (cfg, k)));
                match cohere_opt {
                    Some((cohere_config, key)) => {
                        let url = resolve_cohere_embed_url(cohere_config);
                        let input_type = match is_query {
                            true => "search_query",
                            false => "search_document",
                        };
                        let req = get_shared_client()
                            .post(url)
                            .header("Authorization", format!("bearer {}", key))
                            .header("Content-Type", "application/json")
                            .json(&serde_json::json!({
                                "texts": [text],
                                "model": "embed-english-v3.0",
                                "input_type": input_type
                            }));
                        let vec = post_json_value(req)
                            .await
                            .and_then(|json| {
                                json.pointer("/embeddings")
                                    .and_then(|v| v.as_array())
                                    .and_then(|arr| arr.first())
                                    .and_then(|v| v.as_array())
                                    .map(|arr| parse_f32_array(arr))
                            })
                            .filter(|v| !v.is_empty());
                        match vec {
                            Some(v) => return Ok(v),
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            "openai" => match resolve_openai_embed_credentials(&config) {
                Some((key, openai_base)) => {
                    let url = format!("{}/embeddings", openai_base.trim_end_matches('/'));
                    let req = get_shared_client()
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", key))
                        .json(&serde_json::json!({
                            "input": text,
                            "model": "text-embedding-3-small"
                        }));
                    let vec = post_json_value(req)
                        .await
                        .and_then(|json| {
                            json.pointer("/data")
                                .and_then(|v| v.as_array())
                                .and_then(|data| data.first())
                                .and_then(|first| first.pointer("/embedding"))
                                .and_then(|v| v.as_array())
                                .map(|arr| parse_f32_array(arr))
                        })
                        .filter(|v| !v.is_empty());
                    match vec {
                        Some(v) => return Ok(v),
                        None => {}
                    }
                }
                None => {}
            },
            _ => {}
        }
    }

    Err(anyhow::anyhow!(
        "No active cloud embedding providers succeeded"
    ))
}

pub async fn get_cloud_embeddings_batch(
    queries: Vec<String>,
    is_query: bool,
) -> Result<Vec<Vec<f32>>> {
    match queries.is_empty() {
        true => return Ok(Vec::new()),
        false => {}
    }

    let config = crate::config::loader::load_config()?;
    let providers_order = ordered_embedding_providers(&config);

    for provider_name in providers_order {
        match provider_name {
            "google" => {
                let key_opt = config
                    .providers
                    .google_ai_studio
                    .as_ref()
                    .and_then(resolve_provider_key);
                match key_opt {
                    Some(key) => {
                        let url = format!(
                            "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:batchEmbedContents?key={}",
                            key
                        );
                        let requests: Vec<serde_json::Value> = queries
                            .iter()
                            .map(|q| {
                                serde_json::json!({
                                    "model": "models/text-embedding-004",
                                    "content": {
                                        "parts": [{
                                            "text": q
                                        }]
                                    }
                                })
                            })
                            .collect();
                        let req = get_shared_client().post(&url).json(&serde_json::json!({
                            "requests": requests
                        }));
                        let batch = post_json_value(req)
                            .await
                            .and_then(|json| {
                                json.pointer("/embeddings")
                                    .and_then(|v| v.as_array())
                                    .map(|embeddings| {
                                        embeddings
                                            .iter()
                                            .filter_map(|emb| {
                                                emb.pointer("/values")
                                                    .and_then(|v| v.as_array())
                                                    .map(|arr| parse_f32_array(arr))
                                            })
                                            .collect::<Vec<_>>()
                                    })
                            })
                            .filter(|res| res.len() == queries.len());
                        match batch {
                            Some(res) => return Ok(res),
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            "cohere" => {
                let cohere_opt = config
                    .providers
                    .cohere
                    .as_ref()
                    .and_then(|cfg| resolve_provider_key(cfg).map(|k| (cfg, k)));
                match cohere_opt {
                    Some((cohere_config, key)) => {
                        let url = resolve_cohere_embed_url(cohere_config);
                        let input_type = match is_query {
                            true => "search_query",
                            false => "search_document",
                        };
                        let req = get_shared_client()
                            .post(url)
                            .header("Authorization", format!("bearer {}", key))
                            .header("Content-Type", "application/json")
                            .json(&serde_json::json!({
                                "texts": queries,
                                "model": "embed-english-v3.0",
                                "input_type": input_type
                            }));
                        let batch = post_json_value(req)
                            .await
                            .and_then(|json| {
                                json.pointer("/embeddings")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|item| {
                                                item.as_array().map(|a| parse_f32_array(a))
                                            })
                                            .collect::<Vec<_>>()
                                    })
                            })
                            .filter(|res| res.len() == queries.len());
                        match batch {
                            Some(res) => return Ok(res),
                            None => {}
                        }
                    }
                    None => {}
                }
            }
            "openai" => match resolve_openai_embed_credentials(&config) {
                Some((key, openai_base)) => {
                    let url = format!("{}/embeddings", openai_base.trim_end_matches('/'));
                    let req = get_shared_client()
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", key))
                        .json(&serde_json::json!({
                            "input": queries,
                            "model": "text-embedding-3-small"
                        }));
                    let batch = post_json_value(req)
                        .await
                        .and_then(|json| {
                            json.pointer("/data")
                                .and_then(|v| v.as_array())
                                .map(|data| {
                                    data.iter()
                                        .filter_map(|item| {
                                            item.pointer("/embedding")
                                                .and_then(|v| v.as_array())
                                                .map(|arr| parse_f32_array(arr))
                                        })
                                        .collect::<Vec<_>>()
                                })
                        })
                        .filter(|res| res.len() == queries.len());
                    match batch {
                        Some(res) => return Ok(res),
                        None => {}
                    }
                }
                None => {}
            },
            _ => {}
        }
    }

    Err(anyhow::anyhow!(
        "No active cloud embedding providers succeeded for batch"
    ))
}

pub async fn get_embedding(text: &str, is_query: bool) -> Result<Vec<f32>> {
    let config = crate::config::loader::load_config().ok();
    let mode = config
        .as_ref()
        .and_then(|c| c.embeddings.as_ref())
        .map(|e| e.mode.as_str())
        .unwrap_or("local");

    match mode != "local" {
        true => match get_cloud_embedding(text, is_query).await {
            Ok(vec) => return Ok(vec),
            Err(e) => match mode {
                "cloud_only" => {
                    return Err(anyhow::anyhow!(
                        "Cloud embedding failed and local model fallback is disabled: {:?}",
                        e
                    ));
                }
                _ => {
                    tracing::warn!(
                        "Cloud embedding failed: {:?}. Falling back to local fastembed.",
                        e
                    );
                }
            },
        },
        false => {}
    }

    // Fall back to local ONNX model
    let text_owned = text.to_string();
    tokio::task::spawn_blocking(move || -> Result<Vec<f32>> {
        with_model(|model| {
            let formatted = match is_query {
                true => format!("query: {}", text_owned),
                false => format!("passage: {}", text_owned),
            };
            let embeds = model.embed(vec![&formatted], None)?;
            Ok(embeds[0].clone())
        })
    })
    .await?
}

pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for i in 0..std::cmp::min(v1.len(), v2.len()) {
        dot_product += v1[i] * v2[i];
        norm_a += v1[i] * v1[i];
        norm_b += v2[i] * v2[i];
    }
    match norm_a == 0.0 || norm_b == 0.0 {
        true => 0.0,
        false => dot_product / (norm_a.sqrt() * norm_b.sqrt()),
    }
}

#[cfg(test)]
#[path = "embeddings_tests.rs"]
mod tests;

