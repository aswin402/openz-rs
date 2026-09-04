use crate::agent::style::colors::{AURA_GOLD, AURA_PURPLE, COLOR_RESET, EMERALD_GREEN};
use crate::config::schema::Config;
use crate::providers::LLMProvider;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::Arc;

pub struct CompilerAutoHealTool {
    pub config: Config,
    pub provider: Arc<dyn LLMProvider>,
}

#[async_trait::async_trait]
impl Tool for CompilerAutoHealTool {
    fn name(&self) -> &str {
        "compiler_auto_heal"
    }

    fn description(&self) -> &str {
        "Run an edit-compile reflection loop to modify a file and automatically resolve compiler/syntax/dependency errors until compilation succeeds."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path of the file to modify or fix."
                },
                "instruction": {
                    "type": "string",
                    "description": "Detailed instruction/description of the edit or fix to apply to the file."
                },
                "compile_command": {
                    "type": "string",
                    "description": "The command line string used to check/verify compilation (e.g. 'cargo check', 'npm run build')."
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Maximum number of compile-and-fix iteration retries (default: 3, max: 5)."
                }
            },
            "required": ["file_path", "instruction", "compile_command"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path_str = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'file_path' argument"))?;
        let instruction = arguments
            .get("instruction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'instruction' argument"))?;
        let compile_command = arguments
            .get("compile_command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'compile_command' argument"))?;
        let max_iterations = arguments
            .get("max_iterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(5) as usize;

        // Validate compile command against allowlist to prevent shell injection
        let allowed_prefixes = [
            "cargo ", "rustc ", "gcc ", "g++ ", "clang ", "clang++ ", "make ", "cmake ", "ninja ",
            "go ", "go build", "go test", "go run", "npm ", "npx ", "yarn ", "pnpm ", "node ",
            "python ", "python3 ", "pip ", "pip3 ", "javac ", "java ", "mvn ", "gradle ", "swift ",
            "swiftc ", "dotnet ", "msbuild ", "pytest ", "jest ", "vitest ", "tsc ",
        ];
        let cmd_lower = compile_command.trim().to_lowercase();
        let is_allowed = allowed_prefixes.iter().any(|p| cmd_lower.starts_with(p));
        if !is_allowed {
            return Err(anyhow!(
                "Compile command '{}' is not in the allowed list. \
                 Allowed: cargo, rustc, gcc, g++, clang, make, cmake, ninja, go, npm, npx, yarn, node, python, javac, java, mvn, gradle, swift, dotnet, pytest, jest, tsc",
                compile_command
            ));
        }

        let file_path = crate::config::resolve_path(file_path_str);
        if !file_path.exists() {
            return Err(anyhow!("File does not exist: {:?}", file_path));
        }

        // Create backup before modification
        let backup_path = file_path.with_extension(format!(
            "{}.bak",
            file_path.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        if let Err(e) = std::fs::copy(&file_path, &backup_path) {
            tracing::warn!("Failed to create backup of {:?}: {}", file_path, e);
        }

        // 1. Read initial file content
        let mut file_content = std::fs::read_to_string(&file_path)?;

        let mut current_error = String::new();
        let mut iteration = 0;
        let mut compile_success = false;

        let system_prompt = "You are an expert compiler auto-healing agent. Your goal is to modify the provided file content based on the instruction and ensure it compiles without errors.\n\
        You must return the COMPLETE updated file content. Do not truncate, do not use comments for unchanged parts, do not output any explanation text or greetings.\n\
        Output your response inside a single markdown code block (e.g. ```rust ... ``` or ```javascript ... ```).";

        while iteration < max_iterations {
            iteration += 1;
            crate::tui_println!(
                "{}🔧 [Compiler Auto-Heal] Iteration {}/{} for {}...{}",
                AURA_PURPLE,
                iteration,
                max_iterations,
                file_path.file_name().unwrap_or_default().to_string_lossy(),
                COLOR_RESET
            );

            // Construct prompt
            let user_prompt = if current_error.is_empty() {
                format!(
                    "TARGET FILE: {:?}\n\n\
                     CURRENT CONTENT:\n\
                     ```\n\
                     {}\n\
                     ```\n\n\
                     INSTRUCTION: {}\n\n\
                     Please edit the file content to satisfy the instruction, and output the complete new file content.",
                    file_path, file_content, instruction
                )
            } else {
                format!(
                    "TARGET FILE: {:?}\n\n\
                     CURRENT CONTENT:\n\
                     ```\n\
                     {}\n\
                     ```\n\n\
                     The previous code failed compilation with the following error:\n\
                     ```\n\
                     {}\n\
                     ```\n\n\
                     Please fix this compilation error and output the complete corrected file content.",
                    file_path, file_content, current_error
                )
            };

            let messages = vec![crate::session::Message {
                role: "user".to_string(),
                content: user_prompt,
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
                extra: serde_json::Map::new(),
            }];

            let settings = crate::providers::GenerationSettings {
                temperature: 0.1,
                max_tokens: 4096,
                reasoning_effort: None,
            };

            let resp = self
                .provider
                .chat(system_prompt, &messages, &[], &settings)
                .await?;
            let response_text = resp
                .content
                .ok_or_else(|| anyhow!("No content returned from AI"))?;

            // Extract the code block
            let mut updated_content = response_text.trim().to_string();
            if updated_content.starts_with("```") {
                let lines: Vec<&str> = updated_content.lines().collect();
                let start = if lines.first().map(|l| l.starts_with("```")).unwrap_or(false) {
                    1
                } else {
                    0
                };
                let end = if lines.last().map(|l| l.starts_with("```")).unwrap_or(false) {
                    lines.len() - 1
                } else {
                    lines.len()
                };
                updated_content = lines[start..end].join("\n");
            }
            let updated_content_str = updated_content;

            // Write to file
            std::fs::write(&file_path, &updated_content_str)?;
            file_content = updated_content_str;

            // Run compile check command
            let mut cmd = if cfg!(target_os = "windows") {
                let mut c = tokio::process::Command::new("cmd");
                c.args(["/C", compile_command]);
                c
            } else {
                let mut c = tokio::process::Command::new("sh");
                c.args(["-c", compile_command]);
                c
            };
            crate::config::loader::set_tokio_command_cwd(&mut cmd);
            let output = cmd.output().await?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if output.status.success() {
                compile_success = true;
                crate::tui_println!(
                    "{}✓ [Compiler Auto-Heal] Compilation succeeded! (Iteration {}){}",
                    EMERALD_GREEN,
                    iteration,
                    COLOR_RESET
                );
                break;
            } else {
                current_error = format!("{}\n{}", stdout, stderr);
                crate::tui_println!(
                    "{}▲ [Compiler Auto-Heal] Compilation failed. Error output captured.{}",
                    AURA_GOLD,
                    COLOR_RESET
                );
            }
        }

        if compile_success {
            Ok(serde_json::json!({
                "status": "success",
                "message": "File edited and compiled successfully",
                "iterations": iteration
            }))
        } else {
            Ok(serde_json::json!({
                "status": "failed",
                "message": "Failed to compile within the maximum number of iterations",
                "error": current_error,
                "iterations": iteration
            }))
        }
    }
}
