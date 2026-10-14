use anyhow::Result;
use crate::channels::notifications::telegram_api_url;
use crate::tools::arguments::{COMMAND_KEYS, PATH_KEYS};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;

static TRUSTED_SESSION_TOOLS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

const FORBIDDEN_DEVICE_BINS: &[&str] = &["dd", "mkfs", "fdisk", "format", "parted"];
const SYSTEM_POWER_BINS: &[&str] = &["shutdown", "reboot", "poweroff", "halt"];
const PRIVILEGE_BINS: &[&str] = &["sudo", "su", "chmod", "chown", "eval", "source"];
const RAW_DELETE_BINS: &[&str] = &["rm", "rmdir", "unlink"];
const NORMAL_DESTRUCTIVE_BINS: &[&str] = &[
    "rm", "rmdir", "unlink", "dd", "mkfs", "fdisk", "format", "parted",
];
const STRICT_XARGS_TARGETS: &[&str] = &["rm", "kill", "chmod", "chown"];
const STRICT_CLEAN_PHRASES: &[&str] = &[
    "cargo clean",
    "npm run clean",
    "bun run clean",
    "yarn clean",
];
const PROCESS_CONTROL_BINS: &[&str] = &["kill", "killall", "pkill"];
const NETWORK_TRANSFER_BINS: &[&str] = &["curl", "wget", "scp", "rsync", "sftp", "nc", "netcat"];
const SHELL_PIPE_TARGETS: &[&str] = &[
    "sh", "bash", "zsh", "dash", "ksh", "fish", "python", "python3", "perl", "ruby", "node", "php",
];
const SHELL_WRAPPER_BINS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh", "su", "sudo"];
const SCRIPT_RUNNER_BINS: &[&str] = &["python", "python3", "perl", "ruby", "node"];
const SEPARATOR_TOKENS: &[&str] = &["&&", "||", ";", "|", "&"];
const REDIRECT_STOP_TOKENS: &[&str] = &[">", ">>", "<", "2>", "2>&1"];

const DANGEROUS_DELETE_LITERALS: &[&str] = &[
    "/", "/*", "~", "~/*", "$home", "$home/*", "${home}", "${home}/*", "/home", "/home/",
];
const SYSTEM_DANGEROUS_PREFIXES: &[&str] = &[
    "/usr", "/etc", "/var", "/bin", "/sbin", "/lib", "/lib64", "/boot", "/sys", "/proc", "/dev",
    "/opt", "/root",
];
const WORKSPACE_PROTECTED_ENTRIES: &[&str] = &[".git", "Cargo.toml", "src", "build.rs"];
const CREDENTIAL_ARG_KEYS: &[&str] = &["credential", "target", "github_token", "gitlab_token"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    Unquoted,
    Single,
    Double,
}

pub struct SecurityGuard;

impl SecurityGuard {
    fn tool_command_arg(arguments: &Value) -> Option<&str> {
        COMMAND_KEYS
            .iter()
            .find_map(|key| arguments.get(*key).and_then(|value| value.as_str()))
    }

    fn is_left_boundary(byte: u8) -> bool {
        (byte as char).is_whitespace()
            || matches!(
                byte,
                b'/' | b';' | b'|' | b'&' | b'`' | b'$' | b'(' | b'\'' | b'"'
            )
    }

    fn is_right_boundary(byte: u8) -> bool {
        (byte as char).is_whitespace()
            || matches!(
                byte,
                b';' | b'|' | b'&' | b'`' | b')' | b',' | b'.' | b'\'' | b'"'
            )
    }

    /// Pure boundary-aware token matcher across all occurrences of `bin` in `cmd`.
    fn has_bin(cmd: &str, bin: &str) -> bool {
        (!bin.is_empty())
            && cmd.match_indices(bin).any(|(idx, _)| {
                let before = idx
                    .checked_sub(1)
                    .and_then(|prev_idx| cmd.as_bytes().get(prev_idx).copied())
                    .map(Self::is_left_boundary)
                    .unwrap_or(true);
                let after = cmd
                    .as_bytes()
                    .get(idx + bin.len())
                    .copied()
                    .map(Self::is_right_boundary)
                    .unwrap_or(true);
                before && after
            })
    }

    fn has_any_bin(cmd: &str, bins: &[&str]) -> bool {
        bins.iter().any(|bin| Self::has_bin(cmd, bin))
    }

    fn has_shell_substitution(cmd: &str) -> bool {
        let bytes = cmd.as_bytes();
        let mut state = QuoteState::Unquoted;
        let mut idx = 0usize;
        while idx < bytes.len() {
            let b = bytes[idx];
            let next = bytes.get(idx + 1).copied();
            match (state, b, next) {
                (QuoteState::Unquoted | QuoteState::Double, b'\\', Some(_)) => {
                    idx += 2;
                    continue;
                }
                (QuoteState::Unquoted, b'\'', _) => state = QuoteState::Single,
                (QuoteState::Single, b'\'', _) => state = QuoteState::Unquoted,
                (QuoteState::Unquoted, b'"', _) => state = QuoteState::Double,
                (QuoteState::Double, b'"', _) => state = QuoteState::Unquoted,
                (QuoteState::Unquoted | QuoteState::Double, b'`', _)
                | (QuoteState::Unquoted | QuoteState::Double, b'$', Some(b'('))
                | (QuoteState::Unquoted, b'<', Some(b'('))
                | (QuoteState::Unquoted, b'>', Some(b'(')) => return true,
                _ => {}
            }
            idx += 1;
        }
        false
    }

    pub(crate) fn matches_whitelisted_prefix(cmd: &str, prefix: &str) -> bool {
        let cmd_trimmed = cmd.trim();
        let prefix_trimmed = prefix.trim();
        (!prefix_trimmed.is_empty())
            && (!Self::has_shell_substitution(cmd_trimmed))
            && (!cmd_trimmed.contains('\n'))
            && (!cmd_trimmed.contains('\r'))
            && cmd_trimmed.starts_with(prefix_trimmed)
            && match cmd_trimmed.as_bytes().get(prefix_trimmed.len()).copied() {
                None => true,
                Some(c) => {
                    (c as char).is_whitespace() || matches!(c, b';' | b'|' | b'&')
                }
            }
    }

    fn resolve_workspace() -> Option<std::path::PathBuf> {
        crate::config::loader::ACTIVE_WORKSPACE
            .try_with(|w| w.clone())
            .ok()
            .or_else(|| std::env::current_dir().ok())
    }

    fn resolve_against_workspace(path_str: &str) -> Option<std::path::PathBuf> {
        let path = std::path::Path::new(path_str);
        match path.is_absolute() {
            true => Some(path.to_path_buf()),
            false => Self::resolve_workspace().map(|ws| ws.join(path)),
        }
    }

    fn is_in_whitelisted_paths(path_str: &str, whitelisted: &[String]) -> bool {
        Self::resolve_against_workspace(path_str)
            .map(|abs_path| {
                let check_path = Self::canonicalize_path(&abs_path);
                whitelisted.iter().any(|wl_path_str| {
                    Self::resolve_against_workspace(wl_path_str)
                        .map(|wl_abs| {
                            let wl_target = wl_abs.canonicalize().unwrap_or(wl_abs);
                            check_path.starts_with(&wl_target)
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    }

    pub(crate) fn is_safe_path(path_str: &str) -> bool {
        let whitelisted = crate::config::loader::load_config()
            .map(|config| {
                Self::is_in_whitelisted_paths(path_str, &config.agents.defaults.whitelisted_paths)
            })
            .unwrap_or(false);
        whitelisted || {
            let resolved = crate::config::loader::resolve_path(path_str);
            crate::config::path_policy::PathPolicy::approval_target()
                .validate(&resolved)
                .is_ok()
        }
    }

    fn canonicalize_path(abs_path: &std::path::Path) -> std::path::PathBuf {
        crate::config::path_policy::canonicalize_with_missing_leaf(abs_path)
    }

    fn expand_delete_target_path(path_str: &str, path_lower: &str) -> Option<std::path::PathBuf> {
        let path = std::path::Path::new(path_str);
        let home_suffix = path_lower
            .strip_prefix("~/")
            .map(|_| &path_str[2..])
            .or_else(|| (path_lower == "~").then_some(""))
            .or_else(|| path_lower.strip_prefix("$home/").map(|_| &path_str[6..]))
            .or_else(|| (path_lower == "$home").then_some(""));

        match (path.is_absolute(), home_suffix) {
            (true, _) => Some(path.to_path_buf()),
            (false, Some(suffix)) => Some(
                dirs::home_dir()
                    .map(|home| home.join(suffix))
                    .unwrap_or_else(|| path.to_path_buf()),
            ),
            (false, None) => Self::resolve_workspace().map(|ws| ws.join(path)),
        }
    }

    fn is_dangerous_delete_path(path_str: &str) -> bool {
        let path_lower = path_str.trim().to_lowercase();
        DANGEROUS_DELETE_LITERALS.contains(&path_lower.as_str())
            || Self::expand_delete_target_path(path_str.trim(), &path_lower)
                .map(|abs_path| {
                    let check_path = Self::canonicalize_path(&abs_path);
                    let path_str_canon = check_path.to_string_lossy();

                    let hits_system = DANGEROUS_DELETE_LITERALS.contains(&path_str_canon.as_ref())
                        || SYSTEM_DANGEROUS_PREFIXES.iter().any(|sys_prefix| {
                            path_str_canon == *sys_prefix
                                || path_str_canon.starts_with(&format!("{sys_prefix}/"))
                        });

                    let hits_home = dirs::home_dir()
                        .and_then(|h| h.canonicalize().ok())
                        .map(|home_canon| {
                            check_path == home_canon
                                || check_path
                                    .parent()
                                    .filter(|parent| *parent == home_canon)
                                    .and_then(|_| check_path.file_name())
                                    .and_then(|n| n.to_str())
                                    .map(|name| name.starts_with('.'))
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false);

                    let hits_workspace = Self::resolve_workspace()
                        .and_then(|ws| ws.canonicalize().ok())
                        .map(|w_canon| {
                            let git_dir = w_canon.join(".git");
                            check_path == w_canon
                                || check_path.starts_with(&git_dir)
                                || WORKSPACE_PROTECTED_ENTRIES
                                    .iter()
                                    .any(|entry| check_path == w_canon.join(entry))
                        })
                        .unwrap_or(false);

                    hits_system || hits_home || hits_workspace
                })
                .unwrap_or(false)
    }

    fn flush_token(args: &mut Vec<String>, current: &mut String) {
        let _ = (!current.is_empty()).then(|| args.push(std::mem::take(current)));
    }

    fn parse_arguments(cmd: &str) -> Vec<String> {
        let mut args = Vec::new();
        let mut current = String::new();
        let mut state = QuoteState::Unquoted;
        let mut chars = cmd.chars().peekable();

        while let Some(c) = chars.next() {
            match (state, c) {
                (QuoteState::Unquoted, '"') => state = QuoteState::Double,
                (QuoteState::Double, '"') => state = QuoteState::Unquoted,
                (QuoteState::Unquoted, '\'') => state = QuoteState::Single,
                (QuoteState::Single, '\'') => state = QuoteState::Unquoted,
                (_, '\\') => {
                    let _ = chars.next().map(|next_c| current.push(next_c));
                }
                (QuoteState::Unquoted, ';' | '\n' | '\r') => {
                    Self::flush_token(&mut args, &mut current);
                    args.push(";".to_string());
                }
                (QuoteState::Unquoted, '|') => {
                    Self::flush_token(&mut args, &mut current);
                    let token = match chars.peek().copied() {
                        Some('|') => {
                            let _ = chars.next();
                            "||"
                        }
                        _ => "|",
                    };
                    args.push(token.to_string());
                }
                (QuoteState::Unquoted, '&') => match (chars.peek().copied(), current.ends_with('>')) {
                    (Some('&'), _) => {
                        Self::flush_token(&mut args, &mut current);
                        let _ = chars.next();
                        args.push("&&".to_string());
                    }
                    (_, true) => current.push(c),
                    (_, false) => {
                        Self::flush_token(&mut args, &mut current);
                        args.push("&".to_string());
                    }
                },
                (QuoteState::Unquoted, ch) => match ch.is_whitespace() {
                    true => Self::flush_token(&mut args, &mut current),
                    false => current.push(ch),
                },
                (_, ch) => current.push(ch),
            }
        }
        Self::flush_token(&mut args, &mut current);
        args
    }

    fn split_commands(args: &[String]) -> Vec<Vec<String>> {
        args.split(|arg| SEPARATOR_TOKENS.contains(&arg.as_str()))
            .filter(|slice| !slice.is_empty())
            .map(|slice| slice.to_vec())
            .collect()
    }

    fn extract_quoted_substrings(script: &str) -> Vec<String> {
        let mut extracted = Vec::new();
        let mut current = String::new();
        let mut active_quote: Option<char> = None;
        let mut chars = script.chars();

        while let Some(c) = chars.next() {
            match (active_quote, c, active_quote == Some(c)) {
                (Some(_), '\\', _) => {
                    let _ = chars.next().map(|next_c| current.push(next_c));
                }
                (Some(_), _, true) => {
                    let trimmed = current.trim();
                    let _ = (!trimmed.is_empty()).then(|| extracted.push(trimmed.to_string()));
                    current.clear();
                    active_quote = None;
                }
                (Some(_), ch, false) => current.push(ch),
                (None, '\'' | '"', _) => active_quote = Some(c),
                (None, _, _) => {}
            }
        }
        extracted
    }

    fn extract_substitutions(cmd: &str) -> Vec<String> {
        let mut extracted = Vec::new();
        let mut chars = cmd.chars().peekable();
        let mut state = QuoteState::Unquoted;

        while let Some(c) = chars.next() {
            match (state, c, chars.peek().copied()) {
                (QuoteState::Unquoted | QuoteState::Double, '\\', Some(_)) => {
                    let _ = chars.next();
                }
                (QuoteState::Unquoted, '\'', _) => state = QuoteState::Single,
                (QuoteState::Single, '\'', _) => state = QuoteState::Unquoted,
                (QuoteState::Unquoted, '"', _) => state = QuoteState::Double,
                (QuoteState::Double, '"', _) => state = QuoteState::Unquoted,
                (QuoteState::Unquoted | QuoteState::Double, '`', _) => {
                    let mut inner = String::new();
                    while let Some(nc) = chars.next() {
                        match nc {
                            '\\' => {
                                let _ = chars.next().map(|esc| inner.push(esc));
                            }
                            '`' => break,
                            other => inner.push(other),
                        }
                    }
                    let trimmed = inner.trim();
                    let _ = (!trimmed.is_empty()).then(|| extracted.push(trimmed.to_string()));
                }
                (QuoteState::Unquoted | QuoteState::Double, '$', Some('('))
                | (QuoteState::Unquoted, '<' | '>', Some('(')) => {
                    let _ = chars.next();
                    let mut inner = String::new();
                    let mut depth = 1usize;
                    while let Some(nc) = chars.next() {
                        match nc {
                            '\\' => {
                                let _ = chars.next().map(|esc| {
                                    inner.push('\\');
                                    inner.push(esc);
                                });
                            }
                            '(' => {
                                depth += 1;
                                inner.push(nc);
                            }
                            ')' => {
                                depth = depth.saturating_sub(1);
                                match depth {
                                    0 => break,
                                    _ => inner.push(nc),
                                }
                            }
                            other => inner.push(other),
                        }
                    }
                    let trimmed = inner.trim();
                    let _ = (!trimmed.is_empty()).then(|| extracted.push(trimmed.to_string()));
                }
                _ => {}
            }
        }
        extracted
    }

    fn is_env_assignment(word: &str) -> bool {
        word.contains('=')
            && !word.starts_with('-')
            && !word.starts_with('/')
            && !word.starts_with('.')
            && !word.starts_with('\\')
    }

    fn first_executable_index(command: &[String]) -> Option<usize> {
        command
            .iter()
            .position(|word| !Self::is_env_assignment(word))
    }

    /// Recursively unwraps command strings wrapped in shell runners (sh -c, bash -c, eval, python -c, etc.)
    /// and command substitutions (`$()`, backticks), returning all outer and inner command strings.
    pub fn unwrap_command_payloads(cmd: &str) -> Vec<String> {
        let mut results = Vec::new();
        let mut stack = vec![(cmd.to_string(), 0usize)];

        while let Some((current_cmd, depth)) = stack.pop() {
            let trimmed = current_cmd.trim();
            let should_expand = (!trimmed.is_empty()) && {
                let _ = (!results.iter().any(|r: &String| r == trimmed))
                    .then(|| results.push(trimmed.to_string()));
                depth < 5
            };

            let _ = should_expand.then(|| {
                for sub in Self::extract_substitutions(trimmed) {
                    stack.push((sub, depth + 1));
                }

                let args = Self::parse_arguments(trimmed);
                for command in Self::split_commands(&args) {
                    let _ = Self::first_executable_index(&command).map(|idx| {
                        let exec_name = command[idx].to_lowercase();
                        let bin_name = exec_name.split('/').next_back().unwrap_or(&exec_name);
                        let tail = &command[idx + 1..];

                        let is_shell = SHELL_WRAPPER_BINS.contains(&bin_name);
                        let is_eval = matches!(bin_name, "eval" | "exec");
                        let is_script = SCRIPT_RUNNER_BINS.contains(&bin_name);

                        let _ = is_shell.then(|| {
                            let _ = tail
                                .windows(2)
                                .find(|pair| matches!(pair[0].as_str(), "-c" | "-lc" | "--command"))
                                .map(|pair| stack.push((pair[1].clone(), depth + 1)));
                        });

                        let _ = (is_eval && !tail.is_empty())
                            .then(|| stack.push((tail.join(" "), depth + 1)));

                        let _ = is_script.then(|| {
                            let _ = tail
                                .windows(2)
                                .find(|pair| matches!(pair[0].as_str(), "-c" | "-e"))
                                .map(|pair| {
                                    let script = &pair[1];
                                    stack.push((script.clone(), depth + 1));
                                    for literal in Self::extract_quoted_substrings(script) {
                                        stack.push((literal, depth + 1));
                                    }
                                });
                        });
                    });
                }
            });
        }

        results
    }

    fn tool_path_arg(arguments: &Value) -> Option<&str> {
        PATH_KEYS
            .iter()
            .find_map(|key| arguments.get(*key).and_then(|value| value.as_str()))
    }

    fn path_edit_label(tool_name: &str) -> Option<&'static str> {
        match tool_name {
            "write_file" => Some("Write File"),
            "patch_file" => Some("Patch File"),
            "replace_lines" => Some("Replace Lines"),
            _ => None,
        }
    }

    /// Check if a tool call is sensitive and needs user approval.
    pub fn is_sensitive(tool_name: &str, arguments: &Value) -> bool {
        Self::is_sensitive_with_mode(tool_name, arguments, "strict")
    }

    fn command_has_forbidden_delete(command: &[String]) -> bool {
        Self::first_executable_index(command)
            .map(|idx| {
                let exec_lower = command[idx].to_lowercase();
                let bin_name = exec_lower.split('/').next_back().unwrap_or(&exec_lower);
                RAW_DELETE_BINS.contains(&bin_name) && {
                    command[idx + 1..]
                        .iter()
                        .take_while(|arg| !REDIRECT_STOP_TOKENS.contains(&arg.as_str()))
                        .scan(false, |check_all, arg| {
                            match (*check_all, arg.as_str(), arg.starts_with('-')) {
                                (false, "--", _) => {
                                    *check_all = true;
                                    Some(None)
                                }
                                (false, _, true) => Some(None),
                                (_, target, _) => Some(Some(target)),
                            }
                        })
                        .flatten()
                        .any(Self::is_dangerous_delete_path)
                }
            })
            .unwrap_or(false)
    }

    /// Check if a command is strictly forbidden and should be rejected instantly without prompting.
    pub fn is_forbidden(tool_name: &str, arguments: &Value) -> bool {
        let canonical_name = crate::tools::canonical_tool_name(tool_name);
        match (canonical_name.as_str(), Self::tool_command_arg(arguments)) {
            ("exec_command", Some(cmd)) => {
                Self::unwrap_command_payloads(cmd).into_iter().any(|payload| {
                    let cmd_lower = payload.to_lowercase();
                    Self::has_any_bin(&cmd_lower, FORBIDDEN_DEVICE_BINS)
                        || Self::has_any_bin(&cmd_lower, SYSTEM_POWER_BINS)
                        || Self::split_commands(&Self::parse_arguments(&payload))
                            .iter()
                            .any(|command| Self::command_has_forbidden_delete(command))
                })
            }
            _ => false,
        }
    }

    fn is_whitelisted_command_line(cmd: &str) -> bool {
        (!Self::has_shell_substitution(cmd))
            && crate::config::loader::load_config()
                .ok()
                .map(|config| {
                    let prefixes = &config.agents.defaults.whitelisted_command_prefixes;
                    let sub_commands = Self::split_commands(&Self::parse_arguments(cmd));
                    (!prefixes.is_empty())
                        && (!sub_commands.is_empty())
                        && sub_commands.iter().all(|sub| {
                            let joined = sub.join(" ");
                            prefixes
                                .iter()
                                .any(|prefix| Self::matches_whitelisted_prefix(&joined, prefix))
                        })
                })
                .unwrap_or(false)
    }

    fn payload_is_sensitive(payload: &str, mode: &str) -> bool {
        let cmd_lower = payload.to_lowercase();
        let always_sensitive = Self::has_any_bin(&cmd_lower, PRIVILEGE_BINS)
            || Self::has_any_bin(&cmd_lower, SYSTEM_POWER_BINS)
            || cmd_lower
                .split('|')
                .skip(1)
                .any(|part| Self::has_any_bin(part, SHELL_PIPE_TARGETS));

        always_sensitive
            || match mode {
                "loose" => false,
                "strict" => {
                    Self::has_any_bin(&cmd_lower, NORMAL_DESTRUCTIVE_BINS)
                        || (Self::has_bin(&cmd_lower, "xargs")
                            && Self::has_any_bin(&cmd_lower, STRICT_XARGS_TARGETS))
                        || STRICT_CLEAN_PHRASES
                            .iter()
                            .any(|phrase| cmd_lower.contains(phrase))
                        || Self::has_any_bin(&cmd_lower, PROCESS_CONTROL_BINS)
                        || Self::has_any_bin(&cmd_lower, NETWORK_TRANSFER_BINS)
                }
                _ => {
                    Self::has_any_bin(&cmd_lower, NORMAL_DESTRUCTIVE_BINS)
                        || (Self::has_bin(&cmd_lower, "xargs")
                            && Self::has_any_bin(&cmd_lower, RAW_DELETE_BINS))
                }
            }
    }

    /// Check if a tool call is sensitive and needs user approval, considering a specific security mode.
    pub fn is_sensitive_with_mode(tool_name: &str, arguments: &Value, security_mode: &str) -> bool {
        let canonical_name = crate::tools::canonical_tool_name(tool_name);
        let tool_name = canonical_name.as_str();
        let mode = security_mode.to_lowercase();

        match tool_name {
            "exec_command" => Self::tool_command_arg(arguments)
                .filter(|cmd| !Self::is_whitelisted_command_line(cmd))
                .map(|cmd| {
                    Self::unwrap_command_payloads(cmd)
                        .iter()
                        .any(|payload| Self::payload_is_sensitive(payload, &mode))
                })
                .unwrap_or(false),
            "manage_config" => {
                let action = arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let has_cred_action = action == "set_credential"
                    || CREDENTIAL_ARG_KEYS
                        .iter()
                        .any(|key| arguments.get(*key).is_some())
                    || Self::contains_secret_material(arguments);
                has_cred_action
                    || crate::tools::ToolMetadata::infer(tool_name).requires_risk_approval()
            }
            _ => {
                let unsafe_path = Self::path_edit_label(tool_name)
                    .and_then(|_| Self::tool_path_arg(arguments))
                    .map(|path_str| !Self::is_safe_path(path_str))
                    .unwrap_or(false);
                unsafe_path || crate::tools::ToolMetadata::infer(tool_name).requires_risk_approval()
            }
        }
    }

    fn contains_secret_material(value: &Value) -> bool {
        crate::core::secrets::contains_secret_material(value)
    }

    fn redacted_value(value: &Value) -> Value {
        crate::core::secrets::redact_secrets_in_json(value)
    }

    pub fn redacted_approval_arguments(arguments: &Value) -> Value {
        Self::redacted_value(arguments)
    }

    /// Formats a concise, human-readable summary of tool arguments for permission prompts.
    pub fn summarize_arguments(arguments: &Value) -> String {
        match arguments {
            Value::Object(map) => {
                if map.is_empty() {
                    return "(no arguments)".to_string();
                }
                let mut parts = Vec::new();
                for (k, v) in map {
                    let val_str = match v {
                        Value::String(s) => {
                            let char_count = s.chars().count();
                            if char_count > 60 {
                                let truncated: String = s.chars().take(60).collect();
                                format!("\"{}...\"", truncated)
                            } else {
                                format!("\"{s}\"")
                            }
                        }
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Array(arr) => {
                            if arr.len() <= 3 && arr.iter().all(|x| x.is_string() || x.is_number()) {
                                let items = arr
                                    .iter()
                                    .map(|x| match x {
                                        Value::String(s) if s.chars().count() > 20 => {
                                            let trunc: String = s.chars().take(20).collect();
                                            format!("\"{}...\"", trunc)
                                        }
                                        _ => x.to_string(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!("[{items}]")
                            } else {
                                format!("[{} items]", arr.len())
                            }
                        }
                        Value::Object(obj) => {
                            format!("{{{} fields}}", obj.len())
                        }
                        Value::Null => "null".to_string(),
                    };
                    parts.push(format!("{k}: {val_str}"));
                }
                parts.join(", ")
            }
            Value::String(s) => {
                let char_count = s.chars().count();
                if char_count > 80 {
                    let truncated: String = s.chars().take(80).collect();
                    format!("\"{}...\"", truncated)
                } else {
                    format!("\"{s}\"")
                }
            }
            other => {
                let s = other.to_string();
                let char_count = s.chars().count();
                if char_count > 80 {
                    let truncated: String = s.chars().take(80).collect();
                    format!("{}...", truncated)
                } else {
                    s
                }
            }
        }
    }

    /// Formats a descriptive string showing the details of the sensitive action.
    pub fn format_description(tool_name: &str, arguments: &Value) -> String {
        let canonical_name = crate::tools::canonical_tool_name(tool_name);
        let tool_name = canonical_name.as_str();
        if tool_name == "exec_command" {
            if let Some(cmd) = Self::tool_command_arg(arguments) {
                return format!("$ {}", cmd);
            }
        } else if tool_name == "manage_config" {
            let redacted = Self::redacted_value(arguments);
            return format!("Manage Config -> {}", redacted);
        } else if let Some(action_label) = Self::path_edit_label(tool_name) {
            let path = Self::tool_path_arg(arguments).unwrap_or("unknown");
            return format!("{} -> {}", action_label, path);
        }
        let redacted = Self::redacted_value(arguments);
        Self::summarize_arguments(&redacted)
    }
}

pub(crate) const APPROVAL_DETAIL_MAX_LINES: usize = 4;
const APPROVAL_DETAIL_MAX_CHARS: usize = 320;

pub(crate) fn compact_approval_description(description: &str, max_width: usize) -> String {
    let mut compact = String::new();
    let mut emitted_lines = 0usize;
    let mut consumed_chars = 0usize;
    let mut truncated = false;

    'outer: for (line_idx, line) in description.lines().enumerate() {
        let wrapped_lines = crate::agent::style::wrap_line(line, max_width);
        for (sub_idx, sub_line) in wrapped_lines.iter().enumerate() {
            let remaining_chars = APPROVAL_DETAIL_MAX_CHARS.saturating_sub(consumed_chars);
            if emitted_lines >= APPROVAL_DETAIL_MAX_LINES || remaining_chars == 0 {
                truncated = true;
                break 'outer;
            }

            let sub_line_chars = sub_line.chars().count();
            let rendered = if sub_line_chars > remaining_chars {
                truncated = true;
                sub_line.chars().take(remaining_chars).collect::<String>()
            } else {
                sub_line.clone()
            };

            if line_idx == 0 && sub_idx == 0 && emitted_lines == 0 {
                compact.push_str(&rendered);
            } else {
                compact.push_str("\n             ");
                compact.push_str(&rendered);
            }
            emitted_lines += 1;
            consumed_chars += rendered.chars().count();

            if sub_line_chars > remaining_chars {
                break 'outer;
            }
        }
    }

    if truncated {
        compact.push_str("\n             ... (details truncated)");
    }

    compact
}

pub fn trust_tool_for_session(session_key: &str, tool_name: &str) {
    let trust_key = format!("{}:{}", session_key, tool_name);
    let map = TRUSTED_SESSION_TOOLS.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut guard) = map.lock() {
        guard.insert(trust_key);
    }
}

pub fn is_tool_trusted_for_session(session_key: &str, tool_name: &str) -> bool {
    let trust_key = format!("{}:{}", session_key, tool_name);
    let map = TRUSTED_SESSION_TOOLS.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(guard) = map.lock() {
        guard.contains(&trust_key)
    } else {
        false
    }
}

/// Request approval for a sensitive tool call over TUI, Telegram, or the WebUI.
pub async fn ask_approval(session_key: &str, tool_name: &str, arguments: &Value) -> Result<bool> {
    if let Some(policy) = crate::cli::headless::current_headless_policy() {
        let permitted = policy.is_tool_permitted(tool_name, true);
        if !permitted {
            policy.record_denial(tool_name);
            tracing::warn!(
                session = %session_key,
                tool = %tool_name,
                "Headless security policy denied execution of sensitive tool"
            );
        }
        return Ok(permitted);
    }

    let description = SecurityGuard::format_description(tool_name, arguments);

    let actual_session = crate::agent::style::spinner::get_current_session_key()
        .unwrap_or_else(|| session_key.to_string());

    if let Some(chat_id_str) = actual_session.strip_prefix("telegram:") {
        // Telegram approval flow
        let chat_id: i64 = chat_id_str.parse()?;

        let (token, client) = match crate::channels::telegram::get_telegram_bot_info() {
            Some(info) => info,
            None => {
                tracing::warn!("Telegram bot info not configured, rejecting sensitive action.");
                return Ok(false);
            }
        };

        let req_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();

        crate::channels::telegram::register_approval(&req_id, tx);

        let send_url = telegram_api_url(&token, "sendMessage");
        let escape_html = |s: &str| -> String {
            s.replace("&", "&amp;")
                .replace("<", "&lt;")
                .replace(">", "&gt;")
        };
        let safe_tool_name = escape_html(tool_name);
        let safe_description = escape_html(&description);

        let text = format!(
            "⚠️ <b>SECURITY ALERT</b>\nOpenZ is requesting to execute a sensitive tool <code>{}</code>:\n<pre>{}</pre>\nDo you approve this action?",
            safe_tool_name, safe_description
        );
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
            "reply_markup": {
                "inline_keyboard": [[
                    { "text": "Approve ✅", "callback_data": format!("approve:{}", req_id) },
                    { "text": "Deny ❌", "callback_data": format!("deny:{}", req_id) }
                ]]
            }
        });

        match client.post(&send_url).json(&payload).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    if let Ok(body) = resp.text().await {
                        tracing::error!(
                            "Failed to send Telegram approval request: status {}, response: {}",
                            status,
                            body
                        );
                    } else {
                        tracing::error!(
                            "Failed to send Telegram approval request: status {}",
                            status
                        );
                    }
                    crate::channels::telegram::unregister_approval(&req_id);
                    return Ok(false);
                }
            }
            Err(e) => {
                tracing::error!("Failed to send Telegram approval request: {:?}", e);
                crate::channels::telegram::unregister_approval(&req_id);
                return Ok(false);
            }
        }

        // Wait for response from user
        match rx.await {
            Ok(approved) => Ok(approved),
            Err(_) => Ok(false),
        }
    } else if crate::agent::style::spinner::is_websocket() || actual_session.starts_with("ws:") {
        // WebUI approval flow: publish a security_request event to the socket
        // that owns this turn and wait for a matching, authenticated response.
        let Some(context) = crate::channels::websocket::current_ws_approval_context() else {
            tracing::warn!(
                session = %actual_session,
                "WebSocket approval requested without an originating client context; denying"
            );
            return Ok(false);
        };
        let req_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        crate::channels::websocket::register_ws_approval(req_id.clone(), context.clone(), tx);

        let delivered = crate::channels::websocket::publish_ws_event_to_client(
            &context.client_id,
            crate::channels::websocket::protocol::security_request(
                context.chat_id.clone(),
                crate::agent::agent_loop::current_turn_id(),
                req_id.clone(),
                tool_name,
                description,
                SecurityGuard::redacted_approval_arguments(arguments),
            ),
        );
        if !delivered {
            let _ = crate::channels::websocket::resolve_ws_approval(
                &req_id,
                &context.client_id,
                &context.chat_id,
                false,
            );
            return Ok(false);
        }

        let approval_result = tokio::time::timeout(std::time::Duration::from_secs(120), rx).await;
        if !matches!(approval_result, Ok(Ok(_))) {
            let _ = crate::channels::websocket::resolve_ws_approval(
                &req_id,
                &context.client_id,
                &context.chat_id,
                false,
            );
        }
        match approval_result {
            Ok(Ok(approved)) => Ok(approved),
            _ => Ok(false),
        }
    } else if actual_session.starts_with("cli:") {
        if is_tool_trusted_for_session(&actual_session, tool_name) {
            return Ok(true); // Automatically approved per session trust decision
        }

        if crate::channels::ratatui::IS_RATATUI_ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
            return crate::channels::ratatui::request_ratatui_security_approval(
                tool_name,
                &description,
                &actual_session,
            )
            .await;
        }

        // CLI / TUI approval flow
        let options = vec![
            "Approve (Allow once)".to_string(),
            "Approve & Trust for this session".to_string(),
            "Deny (Abort tool)".to_string(),
        ];

        let terminal_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80) as usize;
        let max_width = terminal_width.saturating_sub(13).max(10);

        let formatted_details = compact_approval_description(&description, max_width);

        let header = format!(
            "{}🔒 SECURITY SHIELD: Sensitive Action Requested{}\n  {}Tool:      {}{}\n  {}Details:   {}{}",
            crate::agent::style::colors::AURA_GOLD,
            crate::agent::style::colors::COLOR_RESET,
            crate::agent::style::colors::AURA_SLATE,
            crate::agent::style::colors::COLOR_BOLD,
            tool_name,
            crate::agent::style::colors::AURA_SLATE,
            crate::agent::style::colors::AURA_BLUE,
            formatted_details
        );

        // Render minimal themed select menu custom matching the /model command menu
        match crate::agent::style::select_menu_custom(
            "Authorize execution?",
            &options,
            "Security Shield",
            Some(&header),
            false,
        ) {
            Ok(Some(0)) => Ok(true), // Approve once
            Ok(Some(1)) => {
                // Save to trusted set for this session
                trust_tool_for_session(&actual_session, tool_name);
                crate::tui_println!(
                    "{}◇ [Security] Trusted '{}' for session {}.{}",
                    crate::agent::style::colors::AURA_BLUE,
                    tool_name,
                    actual_session,
                    crate::agent::style::colors::COLOR_RESET
                );
                Ok(true)
            }
            _ => Ok(false), // Deny or Cancel
        }
    } else {
        tracing::warn!(
            "Auto-rejecting sensitive action for background session: {}",
            actual_session
        );
        Ok(false)
    }
}

