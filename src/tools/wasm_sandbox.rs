use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;
use wasmtime::*;
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;

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
        let wasm_path_str = arguments.get("wasm_path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'wasm_path' parameter"))?;
        
        let args_val = arguments.get("args").and_then(|v| v.as_array());
        let mut args = Vec::new();
        if let Some(arr) = args_val {
            for arg in arr {
                if let Some(s) = arg.as_str() {
                    args.push(s.to_string());
                }
            }
        }

        let wasm_path = Path::new(wasm_path_str);
        if !wasm_path.exists() {
            return Err(anyhow!("WASM file does not exist: {}", wasm_path_str));
        }

        // Run execution in a spawn_blocking task to not block Tokio runtime
        let path = wasm_path.to_path_buf();
        let res = tokio::task::spawn_blocking(move || {
            execute_wasm(&path, args)
        }).await??;

        Ok(res)
    }
}

pub fn execute_wasm(wasm_path: &Path, args: Vec<String>) -> Result<Value> {
    // 1. Setup engine and compile module
    let engine = Engine::default();
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
        Ok(_) => {
            Ok(json!({
                "status": "success",
                "stdout": stdout_str,
                "stderr": stderr_str,
                "exit_code": 0
            }))
        }
        Err(e) => {
            // Check if it's a WASI exit code trap
            let exit_code = if let Some(_trap) = e.downcast_ref::<Trap>() {
                // In wasmtime 45+, Trap does not hold exit codes directly,
                // but let's check if it's an exit code or just a general error.
                // We'll report the error string.
                1
            } else {
                1
            };
            Ok(json!({
                "status": "error",
                "error": e.to_string(),
                "stdout": stdout_str,
                "stderr": stderr_str,
                "exit_code": exit_code
            }))
        }
    }
}
