use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use crate::providers::{LLMProvider, GenerationSettings};
use crate::agent::style::colors::{AURA_GOLD, EMERALD_GREEN, COLOR_RESET};
use std::sync::Arc;

pub struct CargoManagerTool {
    pub provider: Arc<dyn LLMProvider>,
}

impl CargoManagerTool {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self { provider }
    }
}

async fn run_cargo_cmd(action: &str, cwd: &Option<String>) -> Result<std::process::Output> {
    let mut std_cmd = std::process::Command::new("cargo");

    if let Some(ref cwd_str) = cwd {
        let path = crate::config::loader::resolve_path(cwd_str);
        std_cmd.current_dir(path);
    } else {
        crate::config::loader::set_command_cwd(&mut std_cmd);
    }

    match action {
        "build" => {
            std_cmd.arg("build");
        }
        "test" => {
            std_cmd.arg("test");
        }
        "clippy" => {
            std_cmd.args(["clippy", "--message-format=json"]);
        }
        "fmt" => {
            std_cmd.args(["fmt", "--", "--check"]);
        }
        _ => return Err(anyhow!("Unsupported cargo action: {}", action)),
    }

    let mut tokio_cmd = tokio::process::Command::from(std_cmd);
    tokio_cmd.kill_on_drop(true);
    Ok(tokio_cmd.output().await?)
}

#[async_trait::async_trait]
impl Tool for CargoManagerTool {
    fn name(&self) -> &str {
        "cargo_manager"
    }

    fn description(&self) -> &str {
        "Execute cargo toolchain commands (build, test, clippy, fmt) in a workspace with optional self-healing for compilation errors."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["build", "test", "clippy", "fmt"],
                    "description": "The cargo command to execute."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional working directory to run the cargo command in (defaults to current directory)."
                },
                "self_heal": {
                    "type": "boolean",
                    "description": "If true, automatically attempt to patch and compile any files that generate compiler errors (defaults to true)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;
        let cwd = arguments.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
        let self_heal = arguments.get("self_heal").and_then(|v| v.as_bool()).unwrap_or(true);

        let mut output = run_cargo_cmd(action, &cwd).await?;
        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if action == "build" && !output.status.success() && self_heal {
            let mut retries = 0;
            const MAX_RETRIES: usize = 2;

            while !output.status.success() && retries < MAX_RETRIES {
                // Parse compilation errors to find offending files
                let mut errors_found = Vec::new();
                for line in stderr.lines() {
                    let line_trimmed = line.trim();
                    if let Some(stripped) = line_trimmed.strip_prefix("--> ") {
                        let parts: Vec<&str> = stripped.split(':').collect();
                        if parts.len() >= 2 {
                            let file_path = parts[0].trim().to_string();
                            if let Ok(line_num) = parts[1].trim().parse::<usize>() {
                                errors_found.push((file_path, line_num));
                            }
                        }
                    }
                }

                if errors_found.is_empty() {
                    break;
                }

                crate::tui_println!(
                    "{}◇ [Self-Heal] Compiler error detected in '{}' on line {}. Requesting auto-patch...{}",
                    AURA_GOLD, errors_found[0].0, errors_found[0].1, COLOR_RESET
                );

                // Group by file path and collect errors
                let target_file = &errors_found[0].0;
                let line_num = errors_found[0].1;

                let resolved_path = if let Some(ref cwd_str) = cwd {
                    crate::config::loader::resolve_path(cwd_str).join(target_file)
                } else {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join(target_file)
                };

                if resolved_path.exists() {
                    if let Ok(original_content) = std::fs::read_to_string(&resolved_path) {
                        // Gather relevant error context lines
                        let mut error_context = String::new();
                        let mut collect = false;
                        let mut error_lines_count = 0;
                        for line in stderr.lines() {
                            if line.contains(target_file) && line.contains(&format!(":{}", line_num)) {
                                collect = true;
                                error_lines_count = 0;
                            }
                            if collect {
                                error_context.push_str(line);
                                error_context.push('\n');
                                error_lines_count += 1;
                                if error_lines_count > 12 || (line.trim().is_empty() && error_lines_count > 4) {
                                    collect = false;
                                }
                            }
                        }

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
                            max_tokens: 4096,
                            reasoning_effort: None,
                        };

                        match self.provider.chat(system_prompt, &messages, &[], &settings).await {
                            Ok(resp) => {
                                if let Some(content) = resp.content {
                                    let mut corrected_code = content.trim();
                                    if corrected_code.starts_with("```") {
                                        if let Some(pos) = corrected_code.find('\n') {
                                            corrected_code = &corrected_code[pos+1..];
                                        }
                                    }
                                    if corrected_code.ends_with("```") {
                                        corrected_code = corrected_code[..corrected_code.len() - 3].trim();
                                    }
                                    let corrected_code = corrected_code.trim().to_string();

                                    if !corrected_code.is_empty()
                                        && std::fs::write(&resolved_path, corrected_code).is_ok() {
                                            crate::tui_println!(
                                                "{}✓ [Self-Heal] Successfully patched '{}'. Re-compiling...{}",
                                                EMERALD_GREEN, target_file, COLOR_RESET
                                            );
                                            // Re-run cargo build
                                            output = run_cargo_cmd(action, &cwd).await?;
                                            stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                            stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                        }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Self-heal LLM completion request failed: {:?}", e);
                            }
                        }
                    }
                }

                retries += 1;
            }
        }

        if action == "clippy" {
            let mut diagnostics = Vec::new();
            for line in stdout.lines() {
                if let Ok(msg) = serde_json::from_str::<Value>(line) {
                    if let Some(reason) = msg.get("reason").and_then(|v| v.as_str()) {
                        if reason == "compiler-message" {
                            if let Some(message) = msg.get("message") {
                                let level = message.get("level").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let msg_text = message.get("message").and_then(|v| v.as_str()).unwrap_or("");
                                let spans = message.get("spans").and_then(|v| v.as_array());
                                
                                let mut file_path = String::new();
                                let mut line_num = 0;

                                if let Some(spans_arr) = spans {
                                    if !spans_arr.is_empty() {
                                        let first_span = &spans_arr[0];
                                        file_path = first_span.get("file_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        line_num = first_span.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
                                    }
                                }

                                diagnostics.push(json!({
                                    "level": level,
                                    "message": msg_text,
                                    "file": file_path,
                                    "line": line_num
                                }));
                            }
                        }
                    }
                }
            }

            return Ok(json!({
                "status": if output.status.success() { "success" } else { "error" },
                "diagnostics": diagnostics,
                "code": output.status.code()
            }));
        }

        Ok(json!({
            "status": if output.status.success() { "success" } else { "error" },
            "stdout": stdout,
            "stderr": stderr,
            "code": output.status.code()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cargo_manager() -> Result<()> {
        let provider = Arc::new(crate::providers::openai::OpenAIProvider::new(
            "".to_string(),
            "".to_string(),
            "".to_string(),
        ));
        let tool = CargoManagerTool::new(provider);
        let res = tool.call(&json!({
            "action": "clippy",
            "self_heal": false
        })).await?;

        assert_eq!(res["status"], "success");
        assert!(res["diagnostics"].is_array());
        
        Ok(())
    }
}
