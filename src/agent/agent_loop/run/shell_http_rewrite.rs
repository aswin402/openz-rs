//! Pre-approval normalization of read-only shell HTTP scrape commands (`curl` / `wget`)
//! into native `web_fetch` calls (`url` or concurrent `urls`).

use crate::providers::ToolCallRequest;
use crate::tools::arguments::COMMAND_KEYS;
use crate::tools::routing::contains_phrase_bounded;
use crate::tools::searchxyz::core::crawler::ssrf::validate_url_without_dns;
use std::collections::{HashMap, HashSet};

const MAX_REWRITTEN_URLS: usize = 12;

const EXPLICIT_SHELL_HTTP_TERMS: &[&str] = &["curl", "wget", "exec_command"];

const SAFE_READ_HEADERS: &[&str] = &[
    "user-agent",
    "accept",
    "accept-language",
    "accept-encoding",
    "cache-control",
    "pragma",
    "referer",
    "dnt",
    "upgrade-insecure-requests",
    "sec-fetch-dest",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-user",
    "sec-ch-ua",
    "sec-ch-ua-mobile",
    "sec-ch-ua-platform",
];

#[derive(Debug, Clone)]
struct ShellSegment {
    words: Vec<String>,
    preceded_by_pipe: bool,
}

pub(super) fn web_fetch_call_from_shell_http_scrape(
    call: &ToolCallRequest,
    user_content: &str,
) -> Option<ToolCallRequest> {
    (crate::tools::canonical_tool_name(&call.name) == "exec_command").then_some(())?;
    (!user_explicitly_requests_shell_http(user_content)).then_some(())?;

    let command_str = extract_shell_command(&call.arguments)?;
    let segments = tokenize_shell_segments(command_str)?;
    let urls = extract_scrape_urls_from_segments(&segments)?;

    let arguments = match urls.as_slice() {
        [] => return None,
        [single] => serde_json::json!({ "url": single }),
        many => serde_json::json!({ "urls": many }),
    };

    Some(ToolCallRequest {
        id: call.id.clone(),
        name: "web_fetch".to_string(),
        arguments,
    })
}

fn user_explicitly_requests_shell_http(user_content: &str) -> bool {
    let lower = user_content.to_lowercase();
    EXPLICIT_SHELL_HTTP_TERMS
        .iter()
        .any(|needle| contains_phrase_bounded(&lower, needle))
}

fn extract_shell_command(arguments: &serde_json::Value) -> Option<&str> {
    match arguments {
        serde_json::Value::String(s) => Some(s.trim()).filter(|s| !s.is_empty()),
        serde_json::Value::Object(map) => COMMAND_KEYS
            .iter()
            .find_map(|key| map.get(*key).and_then(|v| v.as_str()))
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn is_safe_external_http_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let lower_prefix = trimmed.get(..8).unwrap_or(trimmed).to_ascii_lowercase();
    let has_http_scheme =
        lower_prefix.starts_with("http://") || lower_prefix.starts_with("https://");
    let has_unexpanded_shell_syntax =
        trimmed.contains('$') || trimmed.chars().any(char::is_whitespace);

    (has_http_scheme && !has_unexpanded_shell_syntax).then_some(())?;
    let parsed = reqwest::Url::parse(trimmed).ok()?;
    validate_url_without_dns(parsed.as_str(), false).ok()?;
    Some(parsed.to_string())
}

fn is_ascii_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => {
            (first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false,
    }
}

fn resolve_url_or_var(
    token: &str,
    var_urls: &HashMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    let trimmed = token.trim();
    let var_lookup = trimmed
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
        .or_else(|| trimmed.strip_prefix('$'));
    match var_lookup {
        Some(var_name) => var_urls.get(var_name).cloned(),
        None => is_safe_external_http_url(trimmed).map(|url| vec![url]),
    }
}

fn is_safe_read_header(header_line: &str) -> bool {
    match header_line.split_once(':') {
        Some((name, _value)) => {
            let normalized = name.trim().to_ascii_lowercase();
            SAFE_READ_HEADERS.contains(&normalized.as_str())
        }
        None => false,
    }
}

fn flush_word(
    current_word: &mut String,
    has_word: &mut bool,
    words: &mut Vec<String>,
) {
    let _ = (*has_word).then(|| {
        words.push(std::mem::take(current_word));
        *has_word = false;
    });
}

fn flush_segment(
    current_word: &mut String,
    has_word: &mut bool,
    words: &mut Vec<String>,
    preceded_by_pipe: &mut bool,
    next_pipe: bool,
    segments: &mut Vec<ShellSegment>,
) {
    flush_word(current_word, has_word, words);
    let _ = (!words.is_empty()).then(|| {
        segments.push(ShellSegment {
            words: std::mem::take(words),
            preceded_by_pipe: *preceded_by_pipe,
        });
    });
    *preceded_by_pipe = next_pipe;
}

fn consume_safe_redirect(chars: &[char], start: usize) -> Option<usize> {
    let mut idx = start;
    let first = *chars.get(idx)?;
    idx += match (first, chars.get(idx + 1).copied()) {
        ('&', Some('>')) => 2,
        ('>', Some('>')) => 2,
        ('>', _) => 1,
        _ => return None,
    };

    match chars.get(idx).copied() {
        Some('&') => {
            idx += 1;
            let fd_char = *chars.get(idx)?;
            let next_boundary = chars
                .get(idx + 1)
                .is_none_or(|c| c.is_whitespace() || matches!(c, ';' | '|' | '&'));
            (matches!(fd_char, '1' | '2') && next_boundary).then_some(idx + 1)
        }
        _ => {
            while chars
                .get(idx)
                .is_some_and(|c| matches!(*c, ' ' | '\t'))
            {
                idx += 1;
            }
            let target_start = idx;
            while chars.get(idx).is_some_and(|c| {
                !c.is_whitespace() && !matches!(*c, ';' | '|' | '&' | '>' | '<' | '\'' | '"')
            }) {
                idx += 1;
            }
            let target: String = chars[target_start..idx].iter().collect();
            (target == "/dev/null").then_some(idx)
        }
    }
}

fn tokenize_shell_segments(cmd: &str) -> Option<Vec<ShellSegment>> {
    let chars: Vec<char> = cmd.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut current_word = String::new();
    let mut has_word = false;
    let mut word_was_quoted = false;
    let mut words: Vec<String> = Vec::new();
    let mut segments: Vec<ShellSegment> = Vec::new();
    let mut preceded_by_pipe = false;

    while i < n {
        let ch = chars[i];
        match (in_single, in_double) {
            (true, _) => {
                match ch == '\'' {
                    true => in_single = false,
                    false => current_word.push(ch),
                }
                i += 1;
            }
            (false, true) => match ch {
                '"' => {
                    in_double = false;
                    i += 1;
                }
                '`' => return None,
                '$' => match chars.get(i + 1).copied() == Some('(') {
                    true => return None,
                    false => {
                        current_word.push(ch);
                        i += 1;
                    }
                },
                '\\' => match chars.get(i + 1).copied() {
                    Some('\n') => i += 2,
                    Some(escaped @ ('"' | '\\' | '$' | '`')) => {
                        current_word.push(escaped);
                        i += 2;
                    }
                    Some(other) => {
                        current_word.push('\\');
                        current_word.push(other);
                        i += 2;
                    }
                    None => return None,
                },
                _ => {
                    current_word.push(ch);
                    i += 1;
                }
            },
            (false, false) => match ch {
                '\'' => {
                    in_single = true;
                    has_word = true;
                    word_was_quoted = true;
                    i += 1;
                }
                '"' => {
                    in_double = true;
                    has_word = true;
                    word_was_quoted = true;
                    i += 1;
                }
                '\\' => match chars.get(i + 1).copied() {
                    Some('\n') => i += 2,
                    Some(escaped) => {
                        current_word.push(escaped);
                        has_word = true;
                        i += 2;
                    }
                    None => return None,
                },
                '`' | '(' | ')' | '<' => return None,
                '$' => match chars.get(i + 1).copied() == Some('(') {
                    true => return None,
                    false => {
                        current_word.push(ch);
                        has_word = true;
                        i += 1;
                    }
                },
                '#' => match has_word {
                    true => {
                        current_word.push(ch);
                        i += 1;
                    }
                    false => {
                        while i < n && chars[i] != '\n' {
                            i += 1;
                        }
                    }
                },
                ' ' | '\t' | '\r' => {
                    flush_word(&mut current_word, &mut has_word, &mut words);
                    word_was_quoted = false;
                    i += 1;
                }
                ';' | '\n' => {
                    flush_segment(
                        &mut current_word,
                        &mut has_word,
                        &mut words,
                        &mut preceded_by_pipe,
                        false,
                        &mut segments,
                    );
                    word_was_quoted = false;
                    i += 1;
                }
                '|' => match chars.get(i + 1).copied() {
                    Some('|') => {
                        flush_segment(
                            &mut current_word,
                            &mut has_word,
                            &mut words,
                            &mut preceded_by_pipe,
                            false,
                            &mut segments,
                        );
                        word_was_quoted = false;
                        i += 2;
                    }
                    Some('&') => {
                        flush_segment(
                            &mut current_word,
                            &mut has_word,
                            &mut words,
                            &mut preceded_by_pipe,
                            true,
                            &mut segments,
                        );
                        word_was_quoted = false;
                        i += 2;
                    }
                    _ => {
                        flush_segment(
                            &mut current_word,
                            &mut has_word,
                            &mut words,
                            &mut preceded_by_pipe,
                            true,
                            &mut segments,
                        );
                        word_was_quoted = false;
                        i += 1;
                    }
                },
                '&' => match chars.get(i + 1).copied() {
                    Some('&') => {
                        flush_segment(
                            &mut current_word,
                            &mut has_word,
                            &mut words,
                            &mut preceded_by_pipe,
                            false,
                            &mut segments,
                        );
                        word_was_quoted = false;
                        i += 2;
                    }
                    Some('>') => {
                        flush_word(&mut current_word, &mut has_word, &mut words);
                        word_was_quoted = false;
                        i = consume_safe_redirect(&chars, i)?;
                    }
                    _ => return None,
                },
                '>' => {
                    let is_fd_prefix = has_word
                        && !word_was_quoted
                        && matches!(current_word.as_str(), "1" | "2");
                    match is_fd_prefix {
                        true => {
                            current_word.clear();
                            has_word = false;
                        }
                        false => {
                            flush_word(&mut current_word, &mut has_word, &mut words);
                            word_was_quoted = false;
                        }
                    }
                    i = consume_safe_redirect(&chars, i)?;
                }
                _ => {
                    current_word.push(ch);
                    has_word = true;
                    i += 1;
                }
            },
        }
    }

    (!in_single && !in_double).then_some(())?;
    flush_segment(
        &mut current_word,
        &mut has_word,
        &mut words,
        &mut preceded_by_pipe,
        false,
        &mut segments,
    );
    (!segments.is_empty()).then_some(segments)
}

fn parse_curl_segment(
    args: &[String],
    var_urls: &HashMap<String, Vec<String>>,
    urls_out: &mut Vec<String>,
) -> bool {
    let mut idx = 0usize;
    let mut segment_urls: Vec<String> = Vec::new();

    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "-s" | "-S" | "-L" | "-f" | "-k" | "-g" | "-N" | "-4" | "-6" | "--silent"
            | "--show-error" | "--location" | "--fail" | "--fail-with-body" | "--compressed"
            | "--insecure" | "--globoff" | "--no-buffer" | "--no-progress-meter"
            | "--http1.1" | "--http2" | "--ipv4" | "--ipv6" | "--tlsv1.2" | "--tlsv1.3" => {
                idx += 1;
            }
            "-m" | "--max-time" | "--connect-timeout" | "--retry" | "--retry-delay"
            | "--retry-max-time" | "--max-redirs" | "-A" | "--user-agent" | "-e"
            | "--referer" => {
                match args.get(idx + 1) {
                    Some(_) => idx += 2,
                    None => return false,
                }
            }
            "-X" | "--request" => match args.get(idx + 1) {
                Some(method) => match method.trim().eq_ignore_ascii_case("GET") {
                    true => idx += 2,
                    false => return false,
                },
                None => return false,
            },
            "-H" | "--header" => match args.get(idx + 1) {
                Some(hdr) => match is_safe_read_header(hdr) {
                    true => idx += 2,
                    false => return false,
                },
                None => return false,
            },
            "--url" => match args.get(idx + 1).and_then(|u| resolve_url_or_var(u, var_urls)) {
                Some(resolved) => {
                    segment_urls.extend(resolved);
                    idx += 2;
                }
                None => return false,
            },
            _ => match arg.starts_with("--") {
                true => {
                    let allowed_eq = arg
                        .split_once('=')
                        .map(|(flag, val)| match flag {
                            "--max-time" | "--connect-timeout" | "--retry" | "--retry-delay"
                            | "--retry-max-time" | "--max-redirs" | "--user-agent"
                            | "--referer" => !val.is_empty(),
                            "--request" => val.trim().eq_ignore_ascii_case("GET"),
                            "--header" => is_safe_read_header(val),
                            "--url" => match resolve_url_or_var(val, var_urls) {
                                Some(resolved) => {
                                    segment_urls.extend(resolved);
                                    true
                                }
                                None => false,
                            },
                            _ => false,
                        })
                        .unwrap_or(false);
                    match allowed_eq {
                        true => idx += 1,
                        false => return false,
                    }
                }
                false => match arg.strip_prefix('-') {
                    Some(short_cluster) => {
                        let is_pure_safe_cluster = !short_cluster.is_empty()
                            && short_cluster.chars().all(|c| {
                                matches!(
                                    c,
                                    's' | 'S' | 'L' | 'f' | 'k' | 'g' | 'N' | '0' | '1' | '2'
                                        | '4' | '6'
                                )
                            });
                        let is_inline_timeout = short_cluster
                            .strip_prefix('m')
                            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.'));
                        let is_inline_get = short_cluster
                            .strip_prefix('X')
                            .is_some_and(|rest| rest.trim().eq_ignore_ascii_case("GET"));
                        match is_pure_safe_cluster || is_inline_timeout || is_inline_get {
                            true => idx += 1,
                            false => return false,
                        }
                    }
                    None => match resolve_url_or_var(arg, var_urls) {
                        Some(resolved) => {
                            segment_urls.extend(resolved);
                            idx += 1;
                        }
                        None => return false,
                    },
                },
            },
        }
    }

    match segment_urls.is_empty() {
        true => false,
        false => {
            urls_out.extend(segment_urls);
            true
        }
    }
}

fn parse_wget_segment(
    args: &[String],
    var_urls: &HashMap<String, Vec<String>>,
    urls_out: &mut Vec<String>,
) -> bool {
    let mut idx = 0usize;
    let mut saw_stdout_dash = false;
    let mut segment_urls: Vec<String> = Vec::new();

    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "-q" | "--quiet" | "-nv" | "--no-verbose" | "--no-check-certificate" | "-4"
            | "-6" => {
                idx += 1;
            }
            "-O-" | "-qO-" | "-SqO-" => {
                saw_stdout_dash = true;
                idx += 1;
            }
            "-O" | "--output-document" => match args.get(idx + 1).map(String::as_str) {
                Some("-") => {
                    saw_stdout_dash = true;
                    idx += 2;
                }
                _ => return false,
            },
            "--output-document=-" => {
                saw_stdout_dash = true;
                idx += 1;
            }
            "-T" | "--timeout" | "-t" | "--tries" | "-U" | "--user-agent"
            | "--max-redirect" => match args.get(idx + 1) {
                Some(_) => idx += 2,
                None => return false,
            },
            "--header" => match args.get(idx + 1) {
                Some(hdr) => match is_safe_read_header(hdr) {
                    true => idx += 2,
                    false => return false,
                },
                None => return false,
            },
            _ => match arg.starts_with('-') {
                true => {
                    let allowed_eq = arg
                        .split_once('=')
                        .map(|(flag, val)| match flag {
                            "--timeout" | "--tries" | "--user-agent" | "--max-redirect" => {
                                !val.is_empty()
                            }
                            "--header" => is_safe_read_header(val),
                            _ => false,
                        })
                        .unwrap_or(false);
                    match allowed_eq {
                        true => idx += 1,
                        false => return false,
                    }
                }
                false => match resolve_url_or_var(arg, var_urls) {
                    Some(resolved) => {
                        segment_urls.extend(resolved);
                        idx += 1;
                    }
                    None => return false,
                },
            },
        }
    }

    match saw_stdout_dash && !segment_urls.is_empty() {
        true => {
            urls_out.extend(segment_urls);
            true
        }
        false => false,
    }
}

fn is_safe_piped_python_filter(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("-c") => args.get(1).is_some_and(|code| {
            let lower = code.to_ascii_lowercase();
            ![
                "open(",
                "write_text",
                "write_bytes",
                "subprocess",
                "os.system",
                "shutil",
                "unlink(",
                "remove(",
                "urllib.request",
                "requests.",
            ]
            .iter()
            .any(|forbidden| lower.contains(forbidden))
        }),
        _ => false,
    }
}

fn is_safe_piped_node_filter(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("-e" | "-p") => args.get(1).is_some_and(|code| {
            let lower = code.to_ascii_lowercase();
            ![
                "writefile",
                "appendfile",
                "unlink",
                "rmsync",
                "child_process",
            ]
            .iter()
            .any(|forbidden| lower.contains(forbidden))
        }),
        _ => false,
    }
}

fn strip_leading_control_keywords(words: &[String]) -> &[String] {
    let mut slice = words;
    while let Some(first) = slice.first().map(String::as_str) {
        match matches!(first, "do" | "then") {
            true => slice = &slice[1..],
            false => break,
        }
    }
    slice
}

fn extract_scrape_urls_from_segments(segments: &[ShellSegment]) -> Option<Vec<String>> {
    let mut var_urls: HashMap<String, Vec<String>> = HashMap::new();
    let mut raw_urls: Vec<String> = Vec::new();
    let mut saw_http_fetch = false;

    for segment in segments {
        let words = strip_leading_control_keywords(&segment.words);
        match words.split_first() {
            None => {}
            Some((first, rest)) => {
                let cmd_name = first.rsplit('/').next().unwrap_or(first.as_str());
                match cmd_name {
                    "for" => {
                        (rest.len() >= 3 && rest[1] == "in" && is_ascii_var_name(&rest[0]))
                            .then_some(())?;
                        let mut bound = Vec::with_capacity(rest.len() - 2);
                        for item in &rest[2..] {
                            bound.push(is_safe_external_http_url(item)?);
                        }
                        var_urls.insert(rest[0].clone(), bound);
                    }
                    "done" | "echo" | "printf" | "true" | "sleep" => {}
                    "curl" => {
                        parse_curl_segment(rest, &var_urls, &mut raw_urls).then_some(())?;
                        saw_http_fetch = true;
                    }
                    "wget" => {
                        parse_wget_segment(rest, &var_urls, &mut raw_urls).then_some(())?;
                        saw_http_fetch = true;
                    }
                    "head" | "tail" | "cat" | "grep" | "egrep" | "fgrep" | "rg" | "tr"
                    | "cut" | "sort" | "uniq" | "wc" | "jq" | "xmllint" | "html2text" => {
                        segment.preceded_by_pipe.then_some(())?;
                    }
                    "sed" => {
                        let has_in_place = rest.iter().any(|arg| {
                            arg == "-i"
                                || arg.starts_with("-i")
                                || arg == "--in-place"
                                || arg.starts_with("--in-place=")
                        });
                        (segment.preceded_by_pipe && !has_in_place).then_some(())?;
                    }
                    "awk" => {
                        let has_system_call = rest.iter().any(|arg| arg.contains("system("));
                        (segment.preceded_by_pipe && !has_system_call).then_some(())?;
                    }
                    "python" | "python3" => {
                        (segment.preceded_by_pipe && is_safe_piped_python_filter(rest))
                            .then_some(())?;
                    }
                    "node" => {
                        (segment.preceded_by_pipe && is_safe_piped_node_filter(rest))
                            .then_some(())?;
                    }
                    _ => match (rest.is_empty(), first.split_once('=')) {
                        (true, Some((var_name, rhs))) => {
                            is_ascii_var_name(var_name).then_some(())?;
                            let valid_url = is_safe_external_http_url(rhs)?;
                            var_urls.insert(var_name.to_string(), vec![valid_url]);
                        }
                        _ => return None,
                    },
                }
            }
        }
    }

    saw_http_fetch.then_some(())?;
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for url in raw_urls {
        let _ = seen.insert(url.clone()).then(|| deduped.push(url));
    }
    (!deduped.is_empty() && deduped.len() <= MAX_REWRITTEN_URLS).then_some(deduped)
}
