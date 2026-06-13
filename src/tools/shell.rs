use crate::tools::Tool;
use anyhow::{Result, anyhow};
use std::process::Command;

pub struct ExecCommandTool;

#[async_trait::async_trait]
impl Tool for ExecCommandTool {
    fn name(&self) -> &str {
        "exec_command"
    }

    fn description(&self) -> &str {
        "Run a shell command on the host system and return its output."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The shell command to execute" }
            },
            "required": ["command"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let command_str = arguments.get("command").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' argument"))?;

        // 1. Try to parse command line to see if it targets a WASM script or skill
        let parsed_args = parse_command_line(command_str);
        if !parsed_args.is_empty() {
            if let Some(wasm_file) = find_wasm_file(&parsed_args[0]) {
                let path = wasm_file.clone();
                let wasm_args = parsed_args[1..].to_vec();
                
                // Execute in spawn_blocking to avoid blocking tokio executor thread
                let wasm_res = tokio::task::spawn_blocking(move || {
                    crate::tools::wasm_sandbox::execute_wasm(&path, wasm_args)
                }).await?;

                match wasm_res {
                    Ok(val) => {
                        let stdout = val.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let stderr = val.get("stderr").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let status_code = val.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        return Ok(serde_json::json!({
                            "status_code": status_code,
                            "stdout": stdout,
                            "stderr": stderr
                        }));
                    }
                    Err(e) => {
                        return Ok(serde_json::json!({
                            "status_code": 1,
                            "stdout": "".to_string(),
                            "stderr": format!("WASM execution error: {}", e)
                        }));
                    }
                }
            }
        }

        // 2. Fallback to standard raw host shell execution
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command_str]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command_str]);
            c
        };
        crate::config::loader::set_command_cwd(&mut cmd);
        let output = cmd.output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let status_code = output.status.code().unwrap_or(-1);

        Ok(serde_json::json!({
            "status_code": status_code,
            "stdout": stdout,
            "stderr": stderr
        }))
    }
}

fn parse_command_line(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut chars = cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            ' ' | '\t' if !in_double_quote && !in_single_quote => {
                if !current.is_empty() {
                    args.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn find_wasm_file(program: &str) -> Option<std::path::PathBuf> {
    let path = crate::config::resolve_path(program);
    
    // Check if the path exists and is a WASM file
    if path.exists() && path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
            return Some(path);
        }
    }

    // Try appending .wasm if not already present
    if path.extension().and_then(|s| s.to_str()) != Some("wasm") {
        let mut wasm_path = path.clone();
        wasm_path.set_extension("wasm");
        if wasm_path.exists() && wasm_path.is_file() {
            return Some(wasm_path);
        }
    }

    // Check the global skills directory (~/.openz/skills/)
    let skills_dir = crate::agent::skills::get_skills_dir();
    if let Some(file_name) = std::path::Path::new(program).file_name() {
        let skill_path = skills_dir.join(file_name);
        if skill_path.exists() && skill_path.is_file() {
            if skill_path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                return Some(skill_path);
            }
        }
        if skill_path.extension().and_then(|s| s.to_str()) != Some("wasm") {
            let mut skill_wasm_path = skill_path.clone();
            skill_wasm_path.set_extension("wasm");
            if skill_wasm_path.exists() && skill_wasm_path.is_file() {
                return Some(skill_wasm_path);
            }
        }

        // Check local skills directory (./skills/)
        let local_skills_dir = std::path::Path::new("skills");
        if local_skills_dir.exists() && local_skills_dir.is_dir() {
            let local_skill_path = local_skills_dir.join(file_name);
            if local_skill_path.exists() && local_skill_path.is_file() {
                if local_skill_path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    return Some(local_skill_path);
                }
            }
            if local_skill_path.extension().and_then(|s| s.to_str()) != Some("wasm") {
                let mut local_skill_wasm_path = local_skill_path.clone();
                local_skill_wasm_path.set_extension("wasm");
                if local_skill_wasm_path.exists() && local_skill_wasm_path.is_file() {
                    return Some(local_skill_wasm_path);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_line() {
        let cmd = "my_program arg1 \"arg 2\" 'arg 3'";
        let args = parse_command_line(cmd);
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "my_program");
        assert_eq!(args[1], "arg1");
        assert_eq!(args[2], "arg 2");
        assert_eq!(args[3], "arg 3");
    }

    #[test]
    fn test_find_wasm_file_nonexistent() {
        let path = find_wasm_file("nonexistent_wasm_file_12345");
        assert!(path.is_none());
    }

    #[tokio::test]
    async fn test_exec_command_fallback() {
        let tool = ExecCommandTool;
        let args = serde_json::json!({
            "command": "echo 'hello openz'"
        });
        let res = tool.call(&args).await.unwrap();
        assert!(res.get("status_code").is_some());
        let stdout = res["stdout"].as_str().unwrap();
        assert!(stdout.contains("hello openz"));
    }

    #[tokio::test]
    async fn test_exec_command_wasm() {
        let temp_dir = std::env::temp_dir();
        let wasm_path = temp_dir.join("test_exec_command_wasm_temp_file_12345.wasm");
        
        let wasm_bytes: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d,
            0x01, 0x00, 0x00, 0x00,
            0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x02, 0x01, 0x00,
            0x07, 0x0a, 0x01, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74, 0x00, 0x00,
            0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
        ];
        
        std::fs::write(&wasm_path, wasm_bytes).unwrap();
        
        let tool = ExecCommandTool;
        let args = serde_json::json!({
            "command": format!("{} arg1 arg2", wasm_path.to_string_lossy())
        });
        
        let res = tool.call(&args).await.unwrap();
        
        // Clean up
        let _ = std::fs::remove_file(wasm_path);
        
        assert_eq!(res["status_code"].as_i64().unwrap(), 0);
        assert!(res.get("stdout").is_some());
        assert!(res.get("stderr").is_some());
    }
}
