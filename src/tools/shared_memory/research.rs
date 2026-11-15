use crate::memory::with_shared_db as with_db;
use crate::tools::arguments::{get_bool_arg, get_u64_arg};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use rusqlite::params;
use serde_json::{json, Value};

use super::db::get_db_mutex;
use super::embeddings::{get_cloud_embeddings_batch, get_embedding, with_model};
use super::research_bundle::{
    clear_indexed_research, delete_indexed_source, encode_embedding_blob,
    export_research_bundle_filtered, import_research_bundle, list_indexed_sources,
    search_research_hybrid, ResearchSearchMode,
};

fn flush_research_chunk(
    query: &str,
    current_heading: &str,
    current_chunk: &mut Vec<&str>,
    current_len: &mut usize,
    chunks: &mut Vec<(String, String)>,
) {
    let chunk_text = current_chunk.join("\n");
    let chunk_query = match current_heading.is_empty() {
        true => query.to_string(),
        false => format!("{} - {}", query, current_heading),
    };
    chunks.push((chunk_query, chunk_text));
    current_chunk.clear();
    *current_len = 0;
}

pub fn chunk_content_by_headings(query: &str, content: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let mut current_heading = String::new();
    let mut current_chunk = Vec::new();
    let mut current_len = 0;

    for line in lines {
        let is_heading = line.trim_start().starts_with('#')
            || line.trim_start().starts_with("--- Sheet:")
            || line.trim_start().starts_with("Title:");

        match is_heading && !current_chunk.is_empty() {
            true => flush_research_chunk(
                query,
                &current_heading,
                &mut current_chunk,
                &mut current_len,
                &mut chunks,
            ),
            false => {}
        }

        match is_heading {
            true => current_heading = line.trim().to_string(),
            false => {}
        }

        current_chunk.push(line);
        current_len += line.len();

        match current_len > 2500 {
            true => flush_research_chunk(
                query,
                &current_heading,
                &mut current_chunk,
                &mut current_len,
                &mut chunks,
            ),
            false => {}
        }
    }

    match current_chunk.is_empty() {
        true => {}
        false => flush_research_chunk(
            query,
            &current_heading,
            &mut current_chunk,
            &mut current_len,
            &mut chunks,
        ),
    }

    match chunks.is_empty() {
        true => chunks.push((query.to_string(), content.to_string())),
        false => {}
    }

    chunks
}

fn infer_url_from_source(source: &str, explicit_url: Option<&str>) -> String {
    explicit_url
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            source
                .strip_prefix("web_fetch: ")
                .map(|u| u.trim().to_string())
        })
        .or_else(|| {
            (source.starts_with("http://") || source.starts_with("https://"))
                .then(|| source.trim().to_string())
        })
        .unwrap_or_default()
}

pub async fn archive_research_entry_with_meta(
    query: &str,
    title: &str,
    content: &str,
    source: &str,
    explicit_url: Option<&str>,
) -> Result<usize> {
    let resolved_url = infer_url_from_source(source, explicit_url);
    let resolved_title = match title.trim().is_empty() {
        true => query.trim(),
        false => title.trim(),
    };
    let chunks = chunk_content_by_headings(query, content);
    let mut stored_chunks = 0usize;

    let mut prepared_rows = Vec::with_capacity(chunks.len());
    for (chunk_query, chunk_content) in chunks {
        let embed_input = format!("{chunk_query}\n{}", chunk_content.chars().take(600).collect::<String>());
        let embedding = get_embedding(&embed_input, false).await.unwrap_or_default();
        let blob = encode_embedding_blob(&embedding);
        let legacy_json = serde_json::to_string(&embedding)?;
        prepared_rows.push((chunk_query, chunk_content, legacy_json, blob));
    }

    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        let tx = conn.transaction()?;
        let timestamp = chrono::Utc::now().to_rfc3339();
        match resolved_url.is_empty() {
            false => {
                tx.execute(
                    "DELETE FROM research_archive_fts WHERE entry_id IN (SELECT id FROM research_archive WHERE url = ?1 AND source = ?2)",
                    params![resolved_url, source],
                )?;
                tx.execute(
                    "DELETE FROM research_archive WHERE url = ?1 AND source = ?2",
                    params![resolved_url, source],
                )?;
            }
            true => {}
        }

        for (chunk_query, chunk_content, legacy_json, blob) in &prepared_rows {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT OR REPLACE INTO research_archive (id, query, title, content, url, source, timestamp, embedding, embedding_blob)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    id,
                    chunk_query,
                    resolved_title,
                    chunk_content,
                    resolved_url,
                    source,
                    timestamp,
                    legacy_json,
                    blob
                ],
            )?;
            tx.execute(
                "INSERT INTO research_archive_fts (entry_id, query, title, content, url, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    id,
                    chunk_query,
                    resolved_title,
                    chunk_content,
                    resolved_url,
                    source
                ],
            )?;
            stored_chunks += 1;
        }
        tx.commit()?;
        Ok(())
    })?;

    Ok(stored_chunks)
}

pub async fn archive_research_entry(query: &str, content: &str, source: &str) -> Result<()> {
    let _ = archive_research_entry_with_meta(query, query, content, source, None).await?;
    Ok(())
}

pub async fn archive_research_entries(entries: Vec<(String, String, String)>) -> Result<()> {
    let mut all_chunks = Vec::new();
    for (query, content, source) in entries {
        let resolved_url = infer_url_from_source(&source, None);
        let chunks = chunk_content_by_headings(&query, &content);
        for (chunk_query, chunk_content) in chunks {
            all_chunks.push((
                chunk_query,
                query.clone(),
                chunk_content,
                resolved_url.clone(),
                source.clone(),
            ));
        }
    }

    match all_chunks.is_empty() {
        true => return Ok(()),
        false => {}
    }

    let config = crate::config::loader::load_config().ok();
    let mode = config
        .as_ref()
        .and_then(|c| c.embeddings.as_ref())
        .map(|e| e.mode.as_str())
        .unwrap_or("local");

    let mut all_embeddings = Vec::with_capacity(all_chunks.len());
    for chunk_group in all_chunks.chunks(128) {
        let queries_to_embed: Vec<String> = chunk_group
            .iter()
            .map(|(q, _, c, _, _)| format!("{q}\n{}", c.chars().take(500).collect::<String>()))
            .collect();

        let mut embeds = None;
        match mode != "local" {
            true => match get_cloud_embeddings_batch(queries_to_embed.clone(), false).await {
                Ok(res) => embeds = Some(res),
                Err(e) => match mode {
                    "cloud_only" => {
                        return Err(anyhow::anyhow!(
                            "Cloud batch embedding failed and local model fallback is disabled: {:?}",
                            e
                        ));
                    }
                    _ => {
                        tracing::warn!(
                            "Cloud batch embedding failed: {:?}. Falling back to local fastembed.",
                            e
                        );
                    }
                },
            },
            false => {}
        }

        let resolved_embeds = match embeds {
            Some(res) => res,
            None => {
                let count = queries_to_embed.len();
                tokio::task::spawn_blocking(move || -> Vec<Vec<f32>> {
                    with_model(|model| {
                        let formatted: Vec<String> = queries_to_embed
                            .iter()
                            .map(|s| format!("passage: {s}"))
                            .collect();
                        let slices: Vec<&str> = formatted.iter().map(String::as_str).collect();
                        model.embed(slices, None)
                    })
                    .unwrap_or_else(|_| vec![Vec::new(); count])
                })
                .await
                .unwrap_or_else(|_| vec![Vec::new(); count])
            }
        };

        all_embeddings.extend(resolved_embeds);
    }

    let _lock = get_db_mutex().lock().await;
    with_db(|conn| {
        let tx = conn.transaction()?;
        let timestamp = chrono::Utc::now().to_rfc3339();
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO research_archive (id, query, title, content, url, source, timestamp, embedding, embedding_blob)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            let mut fts_stmt = tx.prepare(
                "INSERT INTO research_archive_fts (entry_id, query, title, content, url, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;

            for (idx, (chunk_query, title, chunk_content, url, source)) in
                all_chunks.into_iter().enumerate()
            {
                let empty_vec = Vec::new();
                let embedding = all_embeddings.get(idx).unwrap_or(&empty_vec);
                let id = uuid::Uuid::new_v4().to_string();
                let embedding_json = serde_json::to_string(embedding)?;
                let blob = encode_embedding_blob(embedding);

                stmt.execute(params![
                    id,
                    chunk_query,
                    title,
                    chunk_content,
                    url,
                    source,
                    timestamp,
                    embedding_json,
                    blob
                ])?;
                fts_stmt.execute(params![id, chunk_query, title, chunk_content, url, source])?;
            }
        }
        tx.commit()?;
        Ok(())
    })?;
    Ok(())
}

pub async fn search_research_entries(query: &str, top_k: usize) -> Result<Value> {
    let results =
        search_research_hybrid(query, top_k, ResearchSearchMode::Hybrid, None, None).await?;
    Ok(Value::Array(results))
}

fn infer_archive_action(arguments: &Value) -> &'static str {
    let explicit = ["action", "op", "operation", "cmd"]
        .into_iter()
        .find_map(|k| arguments.get(k).and_then(Value::as_str))
        .map(|s| s.trim().to_ascii_lowercase());

    match explicit.as_deref() {
        Some("archive" | "index" | "save" | "store") => "archive",
        Some("delete" | "delete_source" | "remove_source" | "remove") => "delete_source",
        Some("clear" | "clear_index" | "purge") => "clear_index",
        Some("export" | "export_bundle" | "export_research") => "export_bundle",
        Some("import" | "import_bundle" | "import_research") => "import_bundle",
        Some("list" | "list_sources" | "sources") => "list_sources",
        _ => {
            let has_content = ["content", "markdown", "text", "body"]
                .into_iter()
                .any(|k| arguments.get(k).and_then(Value::as_str).is_some_and(|s| !s.is_empty()));
            let has_file_path = [
                "file_path",
                "filePath",
                "path",
                "bundle_path",
                "bundlePath",
                "output_path",
                "outputPath",
                "input_path",
                "inputPath",
            ]
            .into_iter()
            .any(|k| arguments.get(k).and_then(Value::as_str).is_some_and(|s| !s.is_empty()));
            let has_merge = arguments.get("merge").is_some();
            let has_delete_target = ["url_or_prefix", "urlOrPrefix", "prefix"]
                .into_iter()
                .any(|k| arguments.get(k).and_then(Value::as_str).is_some_and(|s| !s.is_empty()));
            let has_confirm = arguments.get("confirm").is_some();

            match (has_content, has_file_path, has_merge, has_delete_target, has_confirm) {
                (true, _, _, _, _) => "archive",
                (false, true, true, _, _) => "import_bundle",
                (false, true, false, _, _) => "export_bundle",
                (false, false, _, true, _) => "delete_source",
                (false, false, _, false, true) => "clear_index",
                _ => "archive",
            }
        }
    }
}

pub struct ArchiveResearchTool;

#[async_trait::async_trait]
impl Tool for ArchiveResearchTool {
    fn name(&self) -> &str {
        "archive_research"
    }

    fn description(&self) -> &str {
        "Archive research content into the unified SQLite FTS5 + vector knowledge base, or manage indexed sources (delete_source, clear_index, export_bundle, import_bundle)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["archive", "list_sources", "delete_source", "clear_index", "export_bundle", "import_bundle"],
                    "description": "Operation to run (default: 'archive'; auto-inferred for legacy bundle/cleanup calls)."
                },
                "query": {
                    "type": "string",
                    "description": "Query, title, or topic describing the research (aliases: title, topic)."
                },
                "content": {
                    "type": "string",
                    "description": "Plain text or Markdown content to archive (aliases: markdown, text)."
                },
                "source": {
                    "type": "string",
                    "description": "Origin tag (e.g. 'web_fetch: URL', 'github', 'manual'). Defaults to 'manual'."
                },
                "url": {
                    "type": "string",
                    "description": "Optional canonical source URL associated with the content."
                },
                "url_or_prefix": {
                    "type": "string",
                    "description": "Exact URL or URL prefix to remove when action='delete_source'."
                },
                "confirm": {
                    "type": "boolean",
                    "description": "Explicit boolean true required for destructive delete_source or clear_index actions."
                },
                "file_path": {
                    "type": "string",
                    "description": "Validated filesystem path for export_bundle or import_bundle (aliases: bundle_path, path, output_path, input_path)."
                },
                "source_filter": {
                    "type": "string",
                    "description": "Optional source/domain/topic substring filter for list_sources or export_bundle (e.g. 'tokio.rs', 'github')."
                },
                "url_prefix": {
                    "type": "string",
                    "description": "Optional URL prefix filter for export_bundle (e.g. 'https://tokio.rs/')."
                },
                "merge": {
                    "type": "boolean",
                    "description": "When importing a research bundle, merge into existing index (default: true)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        match infer_archive_action(arguments) {
            "list_sources" => {
                let source_filter = ["source_filter", "sourceFilter", "source"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str));
                let limit = get_u64_arg(arguments, &["limit", "max_results", "top_k"], 50) as usize;
                let sources = list_indexed_sources(source_filter, limit).await?;
                Ok(json!({
                    "status": "success",
                    "count": sources.len(),
                    "sources": sources,
                }))
            }
            "delete_source" => {
                let target = [
                    "url_or_prefix",
                    "urlOrPrefix",
                    "prefix",
                    "url",
                    "uri",
                    "source",
                ]
                .into_iter()
                .find_map(|k| arguments.get(k).and_then(Value::as_str))
                .ok_or_else(|| anyhow!("Missing 'url_or_prefix' parameter for delete_source"))?;
                let confirm = get_bool_arg(arguments, &["confirm", "confirmed"], false);
                delete_indexed_source(target, confirm).await
            }
            "clear_index" => {
                let confirm = get_bool_arg(arguments, &["confirm", "confirmed"], false);
                clear_indexed_research(confirm).await
            }
            "export_bundle" => {
                let file_path = [
                    "file_path",
                    "filePath",
                    "bundle_path",
                    "bundlePath",
                    "path",
                    "output_path",
                    "outputPath",
                ]
                .into_iter()
                .find_map(|k| arguments.get(k).and_then(Value::as_str))
                .ok_or_else(|| anyhow!("Missing 'file_path' (or 'bundle_path') parameter for export_bundle"))?;
                let source_filter = ["source_filter", "sourceFilter", "domain_filter", "domainFilter"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str));
                let url_prefix = ["url_prefix", "urlPrefix"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str));
                export_research_bundle_filtered(file_path, source_filter, url_prefix).await
            }
            "import_bundle" => {
                let file_path = [
                    "file_path",
                    "filePath",
                    "bundle_path",
                    "bundlePath",
                    "path",
                    "input_path",
                    "inputPath",
                ]
                .into_iter()
                .find_map(|k| arguments.get(k).and_then(Value::as_str))
                .ok_or_else(|| anyhow!("Missing 'file_path' (or 'bundle_path') parameter for import_bundle"))?;
                let merge = get_bool_arg(arguments, &["merge"], true);
                import_research_bundle(file_path, merge).await
            }
            _ => {
                let explicit_url = ["url", "uri"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str));
                let title = ["title", "name"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str))
                    .unwrap_or("");
                let query = ["query", "topic", "title", "url"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str))
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| anyhow!("Missing 'query' or 'title' parameter"))?;
                let content = ["content", "markdown", "text", "body"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str))
                    .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;
                let source = arguments
                    .get("source")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("manual");

                let chunks =
                    archive_research_entry_with_meta(query, title, content, source, explicit_url)
                        .await?;

                Ok(json!({
                    "status": "success",
                    "chunks_indexed": chunks,
                    "message": "Research content archived to the unified SQLite FTS5 + vector knowledge base."
                }))
            }
        }
    }
}

pub struct SearchResearchTool;

#[async_trait::async_trait]
impl Tool for SearchResearchTool {
    fn name(&self) -> &str {
        "search_research"
    }

    fn description(&self) -> &str {
        "Search the unified local research archive using hybrid SQLite FTS5 BM25 + semantic vector RRF, or list indexed research sources."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query to look up in the local research archive (omit or set action='list_sources' to list indexed sources)."
                },
                "top_k": {
                    "type": "integer",
                    "description": "Maximum number of top matches to return (default: 5, aliases: max_results, limit)."
                },
                "mode": {
                    "type": "string",
                    "enum": ["hybrid", "semantic", "keyword"],
                    "description": "Retrieval mode: hybrid FTS5+vector RRF (default), semantic, or keyword."
                },
                "source_filter": {
                    "type": "string",
                    "description": "Optional filter by source tag (e.g. 'github', 'web_fetch', 'spider')."
                },
                "url_prefix": {
                    "type": "string",
                    "description": "Optional URL prefix filter (e.g. 'https://docs.rs/tokio')."
                },
                "action": {
                    "type": "string",
                    "enum": ["search", "list_sources"],
                    "description": "Optional action ('search' default, or 'list_sources')."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let direct_query = arguments.as_str().map(str::trim);
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("search");
        let query = direct_query.or_else(|| {
            ["query", "q", "search", "term", "topic"]
                .into_iter()
                .find_map(|k| arguments.get(k).and_then(Value::as_str))
        });
        let source_filter = ["source_filter", "sourceFilter", "source"]
            .into_iter()
            .find_map(|k| arguments.get(k).and_then(Value::as_str));
        let limit =
            get_u64_arg(arguments, &["top_k", "max_results", "maxResults", "limit"], 5) as usize;

        let should_list = action.eq_ignore_ascii_case("list_sources")
            || query.map(str::trim).unwrap_or("").is_empty();

        match should_list {
            true => {
                let list_cap = get_u64_arg(
                    arguments,
                    &["limit", "max_results", "maxResults", "top_k"],
                    25,
                ) as usize;
                let sources = list_indexed_sources(source_filter, list_cap).await?;
                Ok(json!({
                    "status": "success",
                    "count": sources.len(),
                    "sources": sources,
                }))
            }
            false => {
                let q = query.unwrap_or_default();
                let mode = ResearchSearchMode::parse(
                    arguments.get("mode").and_then(Value::as_str),
                );
                let url_prefix = ["url_prefix", "urlPrefix"]
                    .into_iter()
                    .find_map(|k| arguments.get(k).and_then(Value::as_str));
                let matches =
                    search_research_hybrid(q, limit, mode, source_filter, url_prefix).await?;
                Ok(json!({
                    "status": "success",
                    "count": matches.len(),
                    "matches": matches,
                }))
            }
        }
    }
}
