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

        let file_path = crate::config::resolve_path(file_path_str);
        crate::core::heal::run_compiler_auto_heal(
            self.provider.as_ref(),
            &file_path,
            instruction,
            compile_command,
            max_iterations,
        )
        .await
    }
}

#[cfg(test)]
#[path = "compiler_auto_heal_tests.rs"]
mod tests;
