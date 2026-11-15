use crate::memory::with_shared_db as with_db;
use anyhow::{anyhow, Result};
use reqwest::header;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebCacheMode {
    Auto,
    PreferCache,
    Revalidate,
    Bypass,
}

impl WebCacheMode {
    pub fn from_args(arguments: &serde_json::Value) -> Result<Self> {
        let raw = arguments
            .get("cache_mode")
            .or_else(|| arguments.get("cacheMode"))
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .trim()
            .to_ascii_lowercase();
        match raw.as_str() {
            "auto" | "" => Ok(Self::Auto),
            "prefer_cache" | "prefer-cache" | "cache" => Ok(Self::PreferCache),
            "revalidate" | "validate" | "refresh" => Ok(Self::Revalidate),
            "bypass" | "no_cache" | "no-cache" | "fresh" => Ok(Self::Bypass),
            other => Err(anyhow!(
                "Invalid cache_mode '{}'. Use auto, prefer_cache, revalidate, or bypass.",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedHttpEntry {
    pub url: String,
    pub final_url: String,
    pub body_text: String,
    pub content_type: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub expires_at: String,
    pub status_code: u16,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpCacheStats {
    pub entry_count: usize,
    pub fresh_count: usize,
    pub total_body_bytes: usize,
}

pub fn now_utc() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

pub fn parse_http_date(raw: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc2822(raw)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn cache_control_directives(cache_control: &str) -> Vec<String> {
    cache_control
        .split(',')
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}

fn cache_control_max_age(cache_control: &str) -> Option<i64> {
    cache_control_directives(cache_control)
        .into_iter()
        .find_map(|directive| {
            directive
                .strip_prefix("max-age=")
                .and_then(|raw| raw.trim_matches('"').parse::<i64>().ok())
        })
        .filter(|age| *age >= 0)
}

fn cache_control_has(cache_control: &str, needle: &str) -> bool {
    cache_control_directives(cache_control)
        .iter()
        .any(|directive| directive == needle)
}

pub fn compute_expires_at_with_default_ttl(
    cache_control: Option<&str>,
    last_modified: Option<&str>,
    fetched_at: chrono::DateTime<chrono::Utc>,
    default_ttl_secs: i64,
) -> Option<chrono::DateTime<chrono::Utc>> {
    match cache_control.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cc) => {
            (!cache_control_has(cc, "no-store")).then_some(())?;
            let immediate_revalidate =
                cache_control_has(cc, "no-cache") || cache_control_has(cc, "must-revalidate");
            match immediate_revalidate {
                true => return Some(fetched_at),
                false => {}
            }
            match cache_control_max_age(cc) {
                Some(max_age) => return Some(fetched_at + chrono::Duration::seconds(max_age)),
                None => {}
            }
        }
        None => {}
    }

    last_modified
        .and_then(parse_http_date)
        .map(|lm| {
            let age = fetched_at.signed_duration_since(lm).num_seconds().max(0);
            let heuristic = (age / 10).clamp(60, 86_400);
            fetched_at + chrono::Duration::seconds(heuristic)
        })
        .or_else(|| Some(fetched_at + chrono::Duration::seconds(default_ttl_secs.max(0))))
}

pub fn compute_expires_at(
    cache_control: Option<&str>,
    last_modified: Option<&str>,
    fetched_at: chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    compute_expires_at_with_default_ttl(cache_control, last_modified, fetched_at, 0)
}

pub fn is_cache_fresh(expires_at: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
    chrono::DateTime::parse_from_rfc3339(expires_at)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc) > now)
        .unwrap_or(false)
}

pub fn header_to_string(headers: &header::HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn load_http_cache(url: &str) -> Result<Option<CachedHttpEntry>> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT url, COALESCE(final_url, url), body_text, COALESCE(content_type, 'text/html'), etag, last_modified, expires_at, status_code
             FROM web_fetch_cache
             WHERE url = ?1",
        )?;
        stmt.query_row(params![url], |row| {
            let code: i64 = row.get(7)?;
            Ok(CachedHttpEntry {
                url: row.get(0)?,
                final_url: row.get(1)?,
                body_text: row.get(2)?,
                content_type: row.get(3)?,
                etag: row.get(4)?,
                last_modified: row.get(5)?,
                expires_at: row.get(6)?,
                status_code: u16::try_from(code).unwrap_or(200),
            })
        })
        .optional()
        .map_err(Into::into)
    })
}

pub fn save_http_cache(
    url: &str,
    final_url: &str,
    body_text: &str,
    content_type: &str,
    headers: &header::HeaderMap,
    status_code: u16,
    default_ttl_secs: i64,
) -> Result<()> {
    let fetched_at = now_utc();
    let etag = header_to_string(headers, header::ETAG);
    let last_modified = header_to_string(headers, header::LAST_MODIFIED);
    let cache_control = header_to_string(headers, header::CACHE_CONTROL);
    let expires_at = match compute_expires_at_with_default_ttl(
        cache_control.as_deref(),
        last_modified.as_deref(),
        fetched_at,
        default_ttl_secs,
    ) {
        Some(expires_at) => expires_at,
        None => return Ok(()),
    };
    with_db(|conn| {
        conn.execute(
            "INSERT INTO web_fetch_cache (url, final_url, body_text, content_type, etag, last_modified, cache_control, fetched_at, expires_at, status_code, use_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)
             ON CONFLICT(url) DO UPDATE SET
                 final_url = excluded.final_url,
                 body_text = excluded.body_text,
                 content_type = excluded.content_type,
                 etag = excluded.etag,
                 last_modified = excluded.last_modified,
                 cache_control = excluded.cache_control,
                 fetched_at = excluded.fetched_at,
                 expires_at = excluded.expires_at,
                 status_code = excluded.status_code",
            params![
                url,
                final_url,
                body_text,
                content_type,
                etag,
                last_modified,
                cache_control,
                fetched_at.to_rfc3339(),
                expires_at.to_rfc3339(),
                i64::from(status_code),
            ],
        )?;
        Ok(())
    })
}

pub fn refresh_http_cache_validators(
    url: &str,
    headers: &header::HeaderMap,
    default_ttl_secs: i64,
) -> Result<()> {
    let fetched_at = now_utc();
    let etag = header_to_string(headers, header::ETAG);
    let last_modified = header_to_string(headers, header::LAST_MODIFIED);
    let cache_control = header_to_string(headers, header::CACHE_CONTROL);
    let expires_at = match compute_expires_at_with_default_ttl(
        cache_control.as_deref(),
        last_modified.as_deref(),
        fetched_at,
        default_ttl_secs,
    ) {
        Some(expires_at) => expires_at,
        None => return Ok(()),
    };
    with_db(|conn| {
        conn.execute(
            "UPDATE web_fetch_cache
             SET etag = COALESCE(?2, etag),
                 last_modified = COALESCE(?3, last_modified),
                 cache_control = COALESCE(?4, cache_control),
                 fetched_at = ?5,
                 expires_at = ?6,
                 use_count = use_count + 1
             WHERE url = ?1",
            params![
                url,
                etag,
                last_modified,
                cache_control,
                fetched_at.to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    })
}

pub fn touch_http_cache(url: &str) -> Result<()> {
    with_db(|conn| {
        conn.execute(
            "UPDATE web_fetch_cache SET use_count = use_count + 1 WHERE url = ?1",
            params![url],
        )?;
        Ok(())
    })
}

pub fn http_cache_stats() -> Result<HttpCacheStats> {
    let now_str = now_utc().to_rfc3339();
    with_db(|conn| {
        conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN expires_at > ?1 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(LENGTH(body_text)), 0)
             FROM web_fetch_cache",
            params![now_str],
            |row| {
                Ok(HttpCacheStats {
                    entry_count: row.get::<_, i64>(0)?.max(0) as usize,
                    fresh_count: row.get::<_, i64>(1)?.max(0) as usize,
                    total_body_bytes: row.get::<_, i64>(2)?.max(0) as usize,
                })
            },
        )
        .map_err(Into::into)
    })
}
