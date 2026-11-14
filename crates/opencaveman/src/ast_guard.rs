use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Reason a file or text payload was blocked from compression due to secret exposure risk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRejection {
    pub reason: String,
    pub indicator: String,
}

/// Structural invariants extracted from a Markdown document via `pulldown-cmark` AST walk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuralManifest {
    pub fenced_code_blocks: Vec<String>,
    pub inline_code_spans: Vec<String>,
    pub urls: Vec<String>,
    pub heading_count: usize,
    pub file_paths: Vec<String>,
}

/// Result of comparing structural anchors before and after compression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FidelityReport {
    pub passed: bool,
    pub violations: Vec<String>,
}

const SENSITIVE_FILENAMES: &[&str] = &[
    ".env",
    ".netrc",
    ".npmrc",
    ".pypirc",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    "credentials",
    "credentials.json",
    "secrets.json",
    "secrets.yaml",
    "secrets.yml",
    "secrets.toml",
    "service-account.json",
    "kubeconfig",
];

const SENSITIVE_EXTENSIONS: &[&str] = &[
    "pem", "key", "p12", "pfx", "jks", "keystore", "kdbx", "asc", "gpg",
];

const SECRET_SUBSTRINGS: &[(&str, &str)] = &[
    ("-----BEGIN ", "PEM private key or certificate block"),
    ("AKIA", "AWS access key prefix"),
    ("ASIA", "AWS temporary access key prefix"),
    ("ghp_", "GitHub personal access token"),
    ("github_pat_", "GitHub fine-grained token"),
    ("glpat-", "GitLab personal access token"),
    ("sk-ant-", "Anthropic API key"),
    ("sk-proj-", "OpenAI project API key"),
    ("sk-live-", "Live secret key"),
    ("xoxb-", "Slack bot token"),
    ("xoxp-", "Slack user token"),
];

const CREDENTIAL_ASSIGNMENT_KEYS: &[&str] = &[
    "api_key",
    "apikey",
    "secret_key",
    "private_key",
    "access_token",
    "refresh_token",
    "client_secret",
    "bot_token",
    "database_url",
];

fn shannon_entropy(token: &str) -> f64 {
    let bytes = token.as_bytes();
    let len = bytes.len();
    match len == 0 {
        true => 0.0,
        false => {
            let mut counts = [0usize; 256];
            for &b in bytes {
                counts[b as usize] += 1;
            }
            let total = len as f64;
            counts
                .iter()
                .filter(|&&c| c > 0)
                .map(|&c| {
                    let p = (c as f64) / total;
                    -p * p.log2()
                })
                .sum()
        }
    }
}

fn looks_like_live_secret_value(raw_val: &str) -> bool {
    let trimmed = raw_val
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';'));
    let lower = trimmed.to_ascii_lowercase();
    let is_placeholder = trimmed.len() < 16
        || trimmed.contains(' ')
        || trimmed.starts_with('<')
        || trimmed.starts_with("${")
        || trimmed.starts_with("your_")
        || lower.contains("example")
        || lower.contains("placeholder")
        || lower.contains("redacted")
        || lower.contains("xxx");

    match is_placeholder {
        true => false,
        false => shannon_entropy(trimmed) >= 3.65,
    }
}

fn check_sensitive_path(path: &Path) -> Option<SecretRejection> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let is_env_variant = file_name == ".env"
        || (file_name.starts_with(".env.") && file_name != ".env.example");
    let exact_sensitive = SENSITIVE_FILENAMES.contains(&file_name.as_str());
    let ext_sensitive = SENSITIVE_EXTENSIONS.contains(&ext.as_str());

    match (is_env_variant || exact_sensitive, ext_sensitive) {
        (true, _) => Some(SecretRejection {
            reason: "Refusing to process sensitive credential or environment file".to_string(),
            indicator: file_name,
        }),
        (false, true) => Some(SecretRejection {
            reason: "Refusing to process cryptographic key or keystore extension".to_string(),
            indicator: format!(".{ext}"),
        }),
        (false, false) => None,
    }
}

/// Scans file path and text content for secret material without regular expressions.
pub fn scan_secret_exposure(path: Option<&Path>, content: &str) -> Option<SecretRejection> {
    match path.and_then(check_sensitive_path) {
        Some(rejection) => return Some(rejection),
        None => {}
    }

    for (prefix, label) in SECRET_SUBSTRINGS {
        match content.find(prefix) {
            Some(idx) => {
                let tail = &content[idx..];
                let token: String = tail
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '"' && *c != '\'' && *c != '`')
                    .collect();
                let real_token = *prefix == "-----BEGIN "
                    || (token.len() >= prefix.len() + 10 && shannon_entropy(&token) >= 3.3);
                match real_token {
                    true => {
                        return Some(SecretRejection {
                            reason: format!("Detected {label} in content"),
                            indicator: (*prefix).to_string(),
                        });
                    }
                    false => {}
                }
            }
            None => {}
        }
    }

    for line in content.lines() {
        let sep_pos = line.find('=').or_else(|| line.find(':'));
        match sep_pos {
            Some(pos) => {
                let left = line[..pos]
                    .trim()
                    .trim_matches(|c: char| matches!(c, '"' | '\'' | '-' | '*'))
                    .to_ascii_lowercase();
                let key_matches = CREDENTIAL_ASSIGNMENT_KEYS
                    .iter()
                    .any(|k| left == *k || left.ends_with(k));
                let rhs = &line[pos + 1..];
                match key_matches && looks_like_live_secret_value(rhs) {
                    true => {
                        return Some(SecretRejection {
                            reason: "Detected high-entropy secret assignment in content".to_string(),
                            indicator: left,
                        });
                    }
                    false => {}
                }
            }
            None => {}
        }
    }

    None
}

fn looks_like_file_path_token(raw: &str) -> Option<String> {
    let cleaned = raw.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | ',' | ';' | ':' | '`'
        )
    });
    let cleaned = cleaned.strip_suffix('.').unwrap_or(cleaned);
    let is_url = cleaned.contains("://") || cleaned.starts_with("mailto:");
    let has_slash = cleaned.contains('/');
    let valid_chars = !cleaned.is_empty()
        && cleaned.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~' | '#')
        });

    let has_path_shape = has_slash
        && valid_chars
        && !is_url
        && cleaned.len() >= 4
        && (cleaned.starts_with("./")
            || cleaned.starts_with("../")
            || cleaned.starts_with("~/")
            || cleaned.starts_with('/')
            || cleaned.contains('.'));

    has_path_shape.then(|| cleaned.to_string())
}

/// Extracts structural anchors (code fences, inline code, links, headings, file paths)
/// from Markdown using `pulldown-cmark`.
pub fn extract_structural_anchors(markdown: &str) -> StructuralManifest {
    let mut manifest = StructuralManifest::default();
    let parser = Parser::new_ext(markdown, Options::all());

    let mut in_code_block = false;
    let mut current_code = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                current_code.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                manifest
                    .fenced_code_blocks
                    .push(current_code.trim_end().to_string());
                current_code.clear();
            }
            Event::Start(Tag::Heading { .. }) => {
                manifest.heading_count += 1;
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                let url = dest_url.trim().to_string();
                match url.is_empty() || manifest.urls.contains(&url) {
                    true => {}
                    false => manifest.urls.push(url),
                }
            }
            Event::Code(code_str) => {
                let span = code_str.to_string();
                match manifest.inline_code_spans.contains(&span) {
                    true => {}
                    false => manifest.inline_code_spans.push(span),
                }
            }
            Event::Text(text) => match in_code_block {
                true => current_code.push_str(&text),
                false => {
                    for word in text.split_whitespace() {
                        match word.starts_with("http://") || word.starts_with("https://") {
                            true => {
                                let clean_url = word
                                    .trim_end_matches(|c: char| matches!(c, '.' | ',' | ')' | ';'))
                                    .to_string();
                                match manifest.urls.contains(&clean_url) {
                                    true => {}
                                    false => manifest.urls.push(clean_url),
                                }
                            }
                            false => match looks_like_file_path_token(word) {
                                Some(fp) => match manifest.file_paths.contains(&fp) {
                                    true => {}
                                    false => manifest.file_paths.push(fp),
                                },
                                None => {}
                            },
                        }
                    }
                }
            },
            _ => {}
        }
    }

    manifest
}

/// Verifies that every code fence, inline code literal, URL, heading structure,
/// and file path from `original` survived byte-exact inside `candidate`.
pub fn verify_compression_fidelity(original: &str, candidate: &str) -> FidelityReport {
    let orig = extract_structural_anchors(original);
    let comp = extract_structural_anchors(candidate);
    let mut violations = Vec::new();

    match orig.fenced_code_blocks == comp.fenced_code_blocks {
        true => {}
        false => violations.push(format!(
            "Fenced code block mismatch: expected {} exact block(s), found {}",
            orig.fenced_code_blocks.len(),
            comp.fenced_code_blocks.len()
        )),
    }

    for span in &orig.inline_code_spans {
        let preserved = comp.inline_code_spans.contains(span) || candidate.contains(span.as_str());
        match preserved {
            true => {}
            false => violations.push(format!("Missing inline code literal `{span}`")),
        }
    }

    for url in &orig.urls {
        let preserved = comp.urls.contains(url) || candidate.contains(url.as_str());
        match preserved {
            true => {}
            false => violations.push(format!("Missing URL `{url}`")),
        }
    }

    for fp in &orig.file_paths {
        let preserved = comp.file_paths.contains(fp) || candidate.contains(fp.as_str());
        match preserved {
            true => {}
            false => violations.push(format!("Missing file path `{fp}`")),
        }
    }

    match orig.heading_count > 0 && comp.heading_count == 0 {
        true => violations.push("All Markdown section headings were stripped".to_string()),
        false => {}
    }

    FidelityReport {
        passed: violations.is_empty(),
        violations,
    }
}
