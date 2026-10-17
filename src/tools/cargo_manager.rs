use crate::agent::style::colors::{AURA_GOLD, COLOR_RESET, EMERALD_GREEN};
use crate::providers::{GenerationSettings, LLMProvider};
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct CargoManagerTool {
    pub provider: Arc<dyn LLMProvider>,
}

impl CargoManagerTool {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self { provider }
    }
}

async fn run_cargo_cmd(
    action: &str,
    cwd: &Option<String>,
    extra_args: &[String],
    package: Option<&str>,
) -> Result<std::process::Output> {
    let mut std_cmd = std::process::Command::new("cargo");

    match cwd {
        Some(ref cwd_str) => {
            let path = crate::config::loader::resolve_path(cwd_str);
            std_cmd.current_dir(path);
        }
        None => {
            crate::config::loader::set_command_cwd(&mut std_cmd);
        }
    }

    match action.trim().to_lowercase().as_str() {
        "build" | "b" => {
            std_cmd.arg("build");
        }
        "check" | "c" => {
            std_cmd.arg("check");
        }
        "test" | "t" => {
            std_cmd.arg("test");
        }
        "clippy" => {
            std_cmd.args(["clippy", "--message-format=json"]);
        }
        "fmt" => {
            std_cmd.args(["fmt", "--", "--check"]);
        }
        "clean" => {
            std_cmd.arg("clean");
        }
        "doc" => {
            std_cmd.arg("doc");
        }
        "bench" => {
            std_cmd.arg("bench");
        }
        "run" => {
            std_cmd.arg("run");
        }
        _ => return Err(anyhow!("Unsupported cargo action: {}", action)),
    }

    let _ = package.map(|pkg| std_cmd.args(["-p", pkg]));

    for arg in extra_args {
        std_cmd.arg(arg);
    }

    let mut tokio_cmd = tokio::process::Command::from(std_cmd);
    tokio_cmd.kill_on_drop(true);
    Ok(tokio_cmd.output().await?)
}

fn parse_compiler_errors(stderr: &str) -> Vec<(String, usize)> {
    stderr
        .lines()
        .filter_map(|line| line.trim().strip_prefix("--> "))
        .filter_map(|stripped| {
            let mut parts = stripped.split(':');
            let file_path = parts.next()?.trim().to_string();
            let line_num = parts.next()?.trim().parse::<usize>().ok()?;
            Some((file_path, line_num))
        })
        .collect()
}

fn collect_error_context(stderr: &str, target_file: &str, line_num: usize) -> String {
    let needle = format!(":{line_num}");
    let mut error_context = String::new();
    let mut collect = false;
    let mut error_lines_count = 0usize;
    for line in stderr.lines() {
        let starts_block = line.contains(target_file) && line.contains(&needle);
        match starts_block {
            true => {
                collect = true;
                error_lines_count = 0;
            }
            false => {}
        }
        match collect {
            true => {
                error_context.push_str(line);
                error_context.push('\n');
                error_lines_count += 1;
                let stop = error_lines_count > 12
                    || (line.trim().is_empty() && error_lines_count > 4);
                let _ = stop.then(|| collect = false);
            }
            false => {}
        }
    }
    error_context
}

fn strip_code_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_header = match trimmed.starts_with("```") {
        true => trimmed
            .find('\n')
            .map(|pos| &trimmed[pos + 1..])
            .unwrap_or(trimmed),
        false => trimmed,
    };
    let without_footer = without_header
        .strip_suffix("```")
        .map(str::trim)
        .unwrap_or(without_header);
    without_footer.trim().to_string()
}

fn parse_clippy_diagnostics(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|msg| msg.get("reason").and_then(Value::as_str) == Some("compiler-message"))
        .filter_map(|msg| {
            let message = msg.get("message")?;
            let level = message
                .get("level")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let msg_text = message
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("");
            let first_span = message
                .get("spans")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first());
            let file_path = first_span
                .and_then(|s| s.get("file_name"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let line_num = first_span
                .and_then(|s| s.get("line_start"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            Some(json!({
                "level": level,
                "message": msg_text,
                "file": file_path,
                "line": line_num
            }))
        })
        .collect()
}

#[async_trait::async_trait]
impl Tool for CargoManagerTool {
    fn name(&self) -> &str {
        "cargo_manager"
    }

    fn description(&self) -> &str {
        "Execute cargo toolchain commands (check, build, test, clippy, fmt, clean, doc, bench) in a workspace with optional self-healing for compilation errors."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["check", "build", "test", "clippy", "fmt", "clean", "doc", "bench", "run"],
                    "description": "The cargo command to execute (defaults to 'check')."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory to run the cargo command in (defaults to current directory)."
                },
                "package": {
                    "type": "string",
                    "description": "Optional package name to target with -p <package>."
                },
                "args": {
                    "description": "Optional additional arguments or flags to pass to cargo (array or string).",
                    "type": "array",
                    "items": { "type": "string" }
                },
                "self_heal": {
                    "type": "boolean",
                    "description": "If true, automatically attempt to patch and compile any files that generate compiler errors (defaults to true)."
                }
            }
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| {
                arguments
                    .get("action")
                    .or_else(|| arguments.get("command"))
                    .or_else(|| arguments.get("cmd"))
                    .or_else(|| arguments.get("subcommand"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("check")
                    .to_string()
            });
        let cwd = arguments
            .get("cwd")
            .or_else(|| arguments.get("path"))
            .or_else(|| arguments.get("dir"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let package = arguments
            .get("package")
            .or_else(|| arguments.get("pkg"))
            .or_else(|| arguments.get("package_name"))
            .and_then(|v| v.as_str());

        let mut extra_args = match arguments.get("args").or_else(|| arguments.get("extra_args")) {
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>(),
            Some(Value::String(s)) => s.split_whitespace().map(str::to_string).collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        let add_release = arguments
            .get("release")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !extra_args.iter().any(|a| a == "--release");
        let _ = add_release.then(|| extra_args.push("--release".to_string()));

        let self_heal = arguments
            .get("self_heal")
            .map(|v| {
                v.as_bool().unwrap_or_else(|| {
                    v.as_str()
                        .map(|s| s.trim().eq_ignore_ascii_case("true"))
                        .unwrap_or(true)
                })
            })
            .unwrap_or(true);

        let mut output = run_cargo_cmd(&action, &cwd, &extra_args, package).await?;
        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let can_self_heal = action.eq_ignore_ascii_case("build")
            || action.eq_ignore_ascii_case("check")
            || action.eq_ignore_ascii_case("b")
            || action.eq_ignore_ascii_case("c");

        let mut retries = 0usize;
        const MAX_RETRIES: usize = 2;

        while can_self_heal && self_heal && !output.status.success() && retries < MAX_RETRIES {
            let errors_found = parse_compiler_errors(&stderr);
            let (target_file, line_num) = match errors_found.first() {
                Some(pair) => (&pair.0, pair.1),
                None => break,
            };

            crate::tui_println!(
                "{}◇ [Self-Heal] Compiler error detected in '{}' on line {}. Requesting auto-patch...{}",
                AURA_GOLD,
                target_file,
                line_num,
                COLOR_RESET
            );

            let resolved_path = match cwd {
                Some(ref cwd_str) => crate::config::loader::resolve_path(cwd_str).join(target_file),
                None => std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(target_file),
            };

            let original_opt = resolved_path
                .exists()
                .then(|| std::fs::read_to_string(&resolved_path).ok())
                .flatten();

            match original_opt {
                Some(original_content) => {
                    let error_context = collect_error_context(&stderr, target_file, line_num);
                    let system_prompt = "You are a specialized Rust Compiler Error Resolver.\n\
                        Your task is to fix a compiler error in a Rust source file.\n\
                        Analyze the file content and the compiler errors provided.\n\
                        Correct the code to fix the errors while maintaining all original functionality and styling.\n\
                        Return ONLY the full corrected file content. Do not include any explanation, markdown fences, formatting, or commentary. Just output the raw corrected source code.";

                    let user_prompt = format!(
                        "File: {}\n\nCompiler Errors:\n{}\n\nOriginal File Content:\n{}\n\nPlease provide the full corrected file content.",
                        target_file, error_context, original_content
                    );

                    let messages = vec![crate::session::Message {
                        role: "user".to_string(),
                        content: user_prompt,
                        timestamp: Some(chrono::Utc::now().to_rfc3339()),
                        extra: serde_json::Map::new(),
                    }];

                    let settings = GenerationSettings {
                        temperature: 0.1,
                        max_tokens: 16384,
                        reasoning_effort: None,
                    };

                    match self
                        .provider
                        .chat(system_prompt, &messages, &[], &settings)
                        .await
                    {
                        Ok(resp) => {
                            let corrected_opt = resp
                                .content
                                .as_deref()
                                .map(strip_code_fences)
                                .filter(|code| !code.is_empty());
                            match corrected_opt {
                                Some(corrected_code) => {
                                    let backup_path = resolved_path.with_extension("rs.bak");
                                    let _ = std::fs::copy(&resolved_path, &backup_path);
                                    match std::fs::write(&resolved_path, corrected_code).is_ok() {
                                        true => {
                                            crate::tui_println!(
                                                "{}✓ [Self-Heal] Successfully patched '{}'. Re-compiling...{}",
                                                EMERALD_GREEN,
                                                target_file,
                                                COLOR_RESET
                                            );
                                            output =
                                                run_cargo_cmd(&action, &cwd, &extra_args, package)
                                                    .await?;
                                            stdout =
                                                String::from_utf8_lossy(&output.stdout).to_string();
                                            stderr =
                                                String::from_utf8_lossy(&output.stderr).to_string();
                                            let _ = std::fs::remove_file(&backup_path);
                                        }
                                        false => {
                                            let _ = std::fs::copy(&backup_path, &resolved_path);
                                            let _ = std::fs::remove_file(&backup_path);
                                        }
                                    }
                                }
                                None => {}
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Self-heal LLM completion request failed: {:?}", e);
                        }
                    }
                }
                None => {}
            }

            retries += 1;
        }

        let status_label = match output.status.success() {
            true => "success",
            false => "error",
        };

        match action == "clippy" {
            true => Ok(json!({
                "status": status_label,
                "diagnostics": parse_clippy_diagnostics(&stdout),
                "code": output.status.code()
            })),
            false => Ok(json!({
                "status": status_label,
                "stdout": stdout,
                "stderr": stderr,
                "code": output.status.code()
            })),
        }
    }
}

#[cfg(test)]
#[path = "cargo_manager_tests.rs"]
mod tests;
