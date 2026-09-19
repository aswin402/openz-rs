use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;
use wasmtime::*;
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::WasiCtxBuilder;

pub struct WasmSandboxTool;

#[async_trait::async_trait]
impl Tool for WasmSandboxTool {
    fn name(&self) -> &str {
        "wasm_execute"
    }

    fn description(&self) -> &str {
        "Execute a compiled WebAssembly (.wasm) file in a secure, sandboxed environment. Captures stdout and stderr."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "wasm_path": {
                    "type": "string",
                    "description": "Path to the .wasm file to execute."
                },
                "args": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional command-line arguments to pass to the WebAssembly program."
                }
            },
            "required": ["wasm_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let (raw_path, args) = match arguments {
            Value::String(s) => (s.trim().to_string(), Vec::new()),
            Value::Object(map) => {
                let p = map
                    .get("wasm_path")
                    .or_else(|| map.get("wasmPath"))
                    .or_else(|| map.get("path"))
                    .or_else(|| map.get("file"))
                    .or_else(|| map.get("wasm_file"))
                    .or_else(|| map.get("wasm"))
                    .or_else(|| map.get("target"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .ok_or_else(|| anyhow!("Missing 'wasm_path' parameter"))?;

                let mut args = Vec::new();
                if let Some(args_val) = map.get("args").or_else(|| map.get("arguments")) {
                    if let Some(arr) = args_val.as_array() {
                        for arg in arr {
                            if let Some(s) = arg.as_str() {
                                args.push(s.to_string());
                            } else if arg.is_number() || arg.is_boolean() {
                                args.push(arg.to_string());
                            }
                        }
                    } else if let Some(s) = args_val.as_str() {
                        args.extend(s.split_whitespace().map(|x| x.to_string()));
                    }
                }
                (p, args)
            }
            _ => return Err(anyhow!("Invalid arguments: expected object or wasm path string")),
        };

        if raw_path.is_empty() {
            return Err(anyhow!("Missing or empty 'wasm_path' parameter"));
        }

        let resolved_path = crate::config::resolve_path(&raw_path);
        let final_path = if resolved_path.exists() {
            resolved_path
        } else if !raw_path.ends_with(".wasm") {
            let with_ext = crate::config::resolve_path(&format!("{}.wasm", raw_path));
            if with_ext.exists() {
                with_ext
            } else {
                resolved_path
            }
        } else {
            resolved_path
        };

        if !final_path.exists() {
            return Err(anyhow!("WASM file does not exist: {}", raw_path));
        }
        let metadata = std::fs::metadata(&final_path)?;
        if metadata.len() > 100 * 1024 * 1024 {
            return Err(anyhow!("WASM module too large (max 100MB)"));
        }

        // Run execution in a spawn_blocking task to not block Tokio runtime
        let path = final_path.to_path_buf();
        let res = tokio::task::spawn_blocking(move || execute_wasm(&path, args)).await??;

        Ok(res)
    }
}

pub fn execute_wasm(wasm_path: &Path, args: Vec<String>) -> Result<Value> {
    // 1. Setup engine and compile module
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, wasm_path)?;

    // 2. Setup stdout/stderr capturing pipes
    let stdout = MemoryOutputPipe::new(1024 * 1024); // 1 MB buffer
    let stderr = MemoryOutputPipe::new(1024 * 1024);

    // 3. Configure WASI context
    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder.stdout(stdout.clone());
    wasi_builder.stderr(stderr.clone());

    for arg in args {
        wasi_builder.arg(&arg);
    }

    let wasi_ctx = wasi_builder.build_p1();

    // 4. Create store and linker
    let mut store = Store::new(&engine, wasi_ctx);
    store.set_fuel(1_000_000)?;
    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);

    p1::add_to_linker_sync(&mut linker, |t| t)?;

    // 5. Instantiate and call "_start"
    let instance = linker.instantiate(&mut store, &module)?;
    let start_func = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

    let run_res = start_func.call(&mut store, ());

    // 6. Extract captured outputs
    let stdout_bytes = stdout.contents();
    let stderr_bytes = stderr.contents();

    let stdout_str = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr_str = String::from_utf8_lossy(&stderr_bytes).to_string();

    match run_res {
        Ok(_) => Ok(json!({
            "status": "success",
            "stdout": stdout_str,
            "stderr": stderr_str,
            "exit_code": 0
        })),
        Err(e) => {
            let exit_code = e
                .downcast_ref::<wasmtime_wasi::I32Exit>()
                .map(|exit| exit.0)
                .unwrap_or(1);
            Ok(json!({
                "status": if exit_code == 0 { "success" } else { "error" },
                "error": e.to_string(),
                "stdout": stdout_str,
                "stderr": stderr_str,
                "exit_code": exit_code
            }))
        }
    }
}

#[cfg(test)]
#[path = "wasm_sandbox_tests.rs"]
mod tests;
