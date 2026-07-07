use super::db::get_shared_client;
use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::OnceLock;

static GLOBAL_EMBEDDING_MODEL: OnceLock<std::sync::Mutex<TextEmbedding>> = OnceLock::new();

pub fn get_global_model() -> Result<&'static std::sync::Mutex<TextEmbedding>> {
    if let Some(m) = GLOBAL_EMBEDDING_MODEL.get() {
        Ok(m)
    } else {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))?;
        let _ = GLOBAL_EMBEDDING_MODEL.set(std::sync::Mutex::new(model));
        Ok(GLOBAL_EMBEDDING_MODEL.get().unwrap())
    }
}

async fn get_cloud_embedding(text: &str, is_query: bool) -> Result<Vec<f32>> {
    let config = crate::config::loader::load_config()?;
    let preferred = config
        .embeddings
        .as_ref()
        .and_then(|e| e.preferred_provider.as_deref());

    let mut providers_order = vec!["google", "cohere", "openai"];
    if let Some(pref) = preferred {
        let p_clean = pref.to_lowercase();
        if let Some(pos) = providers_order.iter().position(|&p| {
            p == p_clean
                || (p == "openai" && (p_clean == "opencode_zen" || p_clean == "opencode-zen"))
        }) {
            let removed = providers_order.remove(pos);
            providers_order.insert(0, removed);
        }
    }

    for provider_name in providers_order {
        match provider_name {
            "google" => {
                if let Some(ref google_config) = config.providers.google_ai_studio {
                    let key_opt = google_config
                        .api_key
                        .as_deref()
                        .or_else(|| google_config.extra.get("apiKey").and_then(|v| v.as_str()));
                    if let Some(key) = key_opt {
                        if !key.trim().is_empty() {
                            let url = format!(
                                "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key={}",
                                key
                            );
                            let client = get_shared_client();
                            let res = client
                                .post(&url)
                                .json(&serde_json::json!({
                                    "content": {
                                        "parts": [{
                                            "text": text
                                        }]
                                    }
                                }))
                                .send()
                                .await;
                            if let Ok(res) = res {
                                if res.status().is_success() {
                                    if let Ok(json) = res.json::<serde_json::Value>().await {
                                        if let Some(values) = json
                                            .pointer("/embedding/values")
                                            .and_then(|v| v.as_array())
                                        {
                                            let vec: Vec<f32> = values
                                                .iter()
                                                .filter_map(|v| v.as_f64().map(|f| f as f32))
                                                .collect();
                                            if !vec.is_empty() {
                                                return Ok(vec);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "cohere" => {
                if let Some(ref cohere_config) = config.providers.cohere {
                    let key_opt = cohere_config
                        .api_key
                        .as_deref()
                        .or_else(|| cohere_config.extra.get("apiKey").and_then(|v| v.as_str()));
                    if let Some(key) = key_opt {
                        if !key.trim().is_empty() {
                            let url = "https://api.cohere.com/v1/embed";
                            let client = get_shared_client();
                            let input_type = if is_query {
                                "search_query"
                            } else {
                                "search_document"
                            };
                            let res = client
                                .post(url)
                                .header("Authorization", format!("bearer {}", key))
                                .header("Content-Type", "application/json")
                                .json(&serde_json::json!({
                                    "texts": [text],
                                    "model": "embed-english-v3.0",
                                    "input_type": input_type
                                }))
                                .send()
                                .await;
                            if let Ok(res) = res {
                                if res.status().is_success() {
                                    if let Ok(json) = res.json::<serde_json::Value>().await {
                                        if let Some(arr) =
                                            json.pointer("/embeddings").and_then(|v| v.as_array())
                                        {
                                            if let Some(first_embed) =
                                                arr.first().and_then(|v| v.as_array())
                                            {
                                                let vec: Vec<f32> = first_embed
                                                    .iter()
                                                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                                                    .collect();
                                                if !vec.is_empty() {
                                                    return Ok(vec);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "openai" => {
                let mut openai_key = None;
                let mut openai_base = "https://api.openai.com/v1".to_string();
                let openai_model = "text-embedding-3-small".to_string();

                if let Some(ref opencode_config) = config.providers.opencode_zen {
                    let key_opt = opencode_config
                        .api_key
                        .as_deref()
                        .or_else(|| opencode_config.extra.get("apiKey").and_then(|v| v.as_str()));
                    if let Some(key) = key_opt {
                        if !key.trim().is_empty() {
                            openai_key = Some(key.to_string());
                            let base_opt = opencode_config.api_base.as_deref().or_else(|| {
                                opencode_config
                                    .extra
                                    .get("apiBase")
                                    .and_then(|v| v.as_str())
                            });
                            if let Some(base) = base_opt {
                                openai_base = base.to_string();
                            }
                        }
                    }
                }

                if openai_key.is_none() {
                    if let Some(ref openai_config) = config.providers.openai {
                        let key_opt = openai_config
                            .api_key
                            .as_deref()
                            .or_else(|| openai_config.extra.get("apiKey").and_then(|v| v.as_str()));
                        if let Some(key) = key_opt {
                            if !key.trim().is_empty() {
                                openai_key = Some(key.to_string());
                                let base_opt = openai_config.api_base.as_deref().or_else(|| {
                                    openai_config.extra.get("apiBase").and_then(|v| v.as_str())
                                });
                                if let Some(base) = base_opt {
                                    openai_base = base.to_string();
                                }
                            }
                        }
                    }
                }

                if let Some(key) = openai_key {
                    let url = format!("{}/embeddings", openai_base.trim_end_matches('/'));
                    let client = get_shared_client();
                    let res = client
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", key))
                        .json(&serde_json::json!({
                            "input": text,
                            "model": openai_model
                        }))
                        .send()
                        .await;
                    if let Ok(res) = res {
                        if res.status().is_success() {
                            if let Ok(json) = res.json::<serde_json::Value>().await {
                                if let Some(data) = json.pointer("/data").and_then(|v| v.as_array())
                                {
                                    if let Some(first) = data.first() {
                                        if let Some(values) =
                                            first.pointer("/embedding").and_then(|v| v.as_array())
                                        {
                                            let vec: Vec<f32> = values
                                                .iter()
                                                .filter_map(|v| v.as_f64().map(|f| f as f32))
                                                .collect();
                                            if !vec.is_empty() {
                                                return Ok(vec);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
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
    if queries.is_empty() {
        return Ok(Vec::new());
    }

    let config = crate::config::loader::load_config()?;
    let preferred = config
        .embeddings
        .as_ref()
        .and_then(|e| e.preferred_provider.as_deref());

    let mut providers_order = vec!["google", "cohere", "openai"];
    if let Some(pref) = preferred {
        let p_clean = pref.to_lowercase();
        if let Some(pos) = providers_order.iter().position(|&p| {
            p == p_clean
                || (p == "openai" && (p_clean == "opencode_zen" || p_clean == "opencode-zen"))
        }) {
            let removed = providers_order.remove(pos);
            providers_order.insert(0, removed);
        }
    }

    for provider_name in providers_order {
        match provider_name {
            "google" => {
                if let Some(ref google_config) = config.providers.google_ai_studio {
                    let key_opt = google_config
                        .api_key
                        .as_deref()
                        .or_else(|| google_config.extra.get("apiKey").and_then(|v| v.as_str()));
                    if let Some(key) = key_opt {
                        if !key.trim().is_empty() {
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

                            let client = get_shared_client();
                            let res = client
                                .post(&url)
                                .json(&serde_json::json!({
                                    "requests": requests
                                }))
                                .send()
                                .await;

                            if let Ok(res) = res {
                                if res.status().is_success() {
                                    if let Ok(json) = res.json::<serde_json::Value>().await {
                                        if let Some(embeddings) =
                                            json.pointer("/embeddings").and_then(|v| v.as_array())
                                        {
                                            let mut result = Vec::new();
                                            for emb in embeddings {
                                                if let Some(values) = emb
                                                    .pointer("/values")
                                                    .and_then(|v| v.as_array())
                                                {
                                                    let vec: Vec<f32> = values
                                                        .iter()
                                                        .filter_map(|v| {
                                                            v.as_f64().map(|f| f as f32)
                                                        })
                                                        .collect();
                                                    result.push(vec);
                                                }
                                            }
                                            if result.len() == queries.len() {
                                                return Ok(result);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "cohere" => {
                if let Some(ref cohere_config) = config.providers.cohere {
                    let key_opt = cohere_config
                        .api_key
                        .as_deref()
                        .or_else(|| cohere_config.extra.get("apiKey").and_then(|v| v.as_str()));
                    if let Some(key) = key_opt {
                        if !key.trim().is_empty() {
                            let url = "https://api.cohere.com/v1/embed";
                            let client = get_shared_client();
                            let input_type = if is_query {
                                "search_query"
                            } else {
                                "search_document"
                            };
                            let res = client
                                .post(url)
                                .header("Authorization", format!("bearer {}", key))
                                .header("Content-Type", "application/json")
                                .json(&serde_json::json!({
                                    "texts": queries,
                                    "model": "embed-english-v3.0",
                                    "input_type": input_type
                                }))
                                .send()
                                .await;

                            if let Ok(res) = res {
                                if res.status().is_success() {
                                    if let Ok(json) = res.json::<serde_json::Value>().await {
                                        if let Some(arr) =
                                            json.pointer("/embeddings").and_then(|v| v.as_array())
                                        {
                                            let mut result = Vec::new();
                                            for item in arr {
                                                if let Some(first_embed) = item.as_array() {
                                                    let vec: Vec<f32> = first_embed
                                                        .iter()
                                                        .filter_map(|v| {
                                                            v.as_f64().map(|f| f as f32)
                                                        })
                                                        .collect();
                                                    result.push(vec);
                                                }
                                            }
                                            if result.len() == queries.len() {
                                                return Ok(result);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "openai" => {
                let mut openai_key = None;
                let mut openai_base = "https://api.openai.com/v1".to_string();
                let openai_model = "text-embedding-3-small".to_string();

                if let Some(ref opencode_config) = config.providers.opencode_zen {
                    let key_opt = opencode_config
                        .api_key
                        .as_deref()
                        .or_else(|| opencode_config.extra.get("apiKey").and_then(|v| v.as_str()));
                    if let Some(key) = key_opt {
                        if !key.trim().is_empty() {
                            openai_key = Some(key.to_string());
                            let base_opt = opencode_config.api_base.as_deref().or_else(|| {
                                opencode_config
                                    .extra
                                    .get("apiBase")
                                    .and_then(|v| v.as_str())
                            });
                            if let Some(base) = base_opt {
                                openai_base = base.to_string();
                            }
                        }
                    }
                }

                if openai_key.is_none() {
                    if let Some(ref openai_config) = config.providers.openai {
                        let key_opt = openai_config
                            .api_key
                            .as_deref()
                            .or_else(|| openai_config.extra.get("apiKey").and_then(|v| v.as_str()));
                        if let Some(key) = key_opt {
                            if !key.trim().is_empty() {
                                openai_key = Some(key.to_string());
                                let base_opt = openai_config.api_base.as_deref().or_else(|| {
                                    openai_config.extra.get("apiBase").and_then(|v| v.as_str())
                                });
                                if let Some(base) = base_opt {
                                    openai_base = base.to_string();
                                }
                            }
                        }
                    }
                }

                if let Some(key) = openai_key {
                    let url = format!("{}/embeddings", openai_base.trim_end_matches('/'));
                    let client = get_shared_client();
                    let res = client
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", key))
                        .json(&serde_json::json!({
                            "input": queries,
                            "model": openai_model
                        }))
                        .send()
                        .await;

                    if let Ok(res) = res {
                        if res.status().is_success() {
                            if let Ok(json) = res.json::<serde_json::Value>().await {
                                if let Some(data) = json.pointer("/data").and_then(|v| v.as_array())
                                {
                                    let mut result = Vec::new();
                                    for item in data {
                                        if let Some(values) =
                                            item.pointer("/embedding").and_then(|v| v.as_array())
                                        {
                                            let vec: Vec<f32> = values
                                                .iter()
                                                .filter_map(|v| v.as_f64().map(|f| f as f32))
                                                .collect();
                                            result.push(vec);
                                        }
                                    }
                                    if result.len() == queries.len() {
                                        return Ok(result);
                                    }
                                }
                            }
                        }
                    }
                }
            }
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

    if mode != "local" {
        let cloud_res = get_cloud_embedding(text, is_query).await;
        match cloud_res {
            Ok(vec) => return Ok(vec),
            Err(e) => {
                if mode == "cloud_only" {
                    return Err(anyhow::anyhow!(
                        "Cloud embedding failed and local model fallback is disabled: {:?}",
                        e
                    ));
                }
                tracing::warn!(
                    "Cloud embedding failed: {:?}. Falling back to local fastembed.",
                    e
                );
            }
        }
    }

    // Fall back to local ONNX model
    let text_owned = text.to_string();
    tokio::task::spawn_blocking(move || -> Result<Vec<f32>> {
        let model_mutex = get_global_model()?;
        let mut model = model_mutex
            .lock()
            .map_err(|e| anyhow!("Failed to lock model Mutex: {:?}", e))?;
        let formatted = if is_query {
            format!("query: {}", text_owned)
        } else {
            format!("passage: {}", text_owned)
        };
        let embeds = model.embed(vec![&formatted], None)?;
        Ok(embeds[0].clone())
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
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }
}
