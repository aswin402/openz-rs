use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_codegen::{CodeGenerator, CodegenOptions};
use std::path::Path;

pub struct JsFormatTool;

#[async_trait::async_trait]
impl Tool for JsFormatTool {
    fn name(&self) -> &str {
        "js_format"
    }

    fn description(&self) -> &str {
        "Format and syntax-check JavaScript/TypeScript/JSX/TSX code natively in Rust. Returns clean formatting or syntax errors."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "The JS/TS source code to format and validate."
                },
                "file_path": {
                    "type": "string",
                    "description": "Optional file name or path (e.g. 'index.ts', 'component.tsx') to detect syntax dialect."
                }
            },
            "required": ["code"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let code = arguments.get("code").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'code' parameter"))?;
            
        let file_path = arguments.get("file_path").and_then(|v| v.as_str()).unwrap_or("file.js");
        
        let path = Path::new(file_path);
        let source_type = SourceType::from_path(path)
            .unwrap_or_else(|_| SourceType::default());

        let allocator = Allocator::default();
        let parser_res = Parser::new(&allocator, code, source_type).parse();

        if !parser_res.errors.is_empty() {
            let mut err_msg = String::new();
            for err in parser_res.errors {
                err_msg.push_str(&format!("{:?}\n", err));
            }
            return Ok(json!({
                "status": "error",
                "errors": err_msg,
                "formatted": ""
            }));
        }

        let options = CodegenOptions::default();
        let printed = CodeGenerator::new()
            .with_options(options)
            .build(&parser_res.program);

        Ok(json!({
            "status": "success",
            "errors": "",
            "formatted": printed.source_text
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_js_format_success() {
        let tool = JsFormatTool;
        let args = json!({
            "code": "const a:number=1;function foo(){return a;}",
            "file_path": "test.ts"
        });
        
        let res = tool.call(&args).await.unwrap();
        assert_eq!(res["status"], "success");
        let formatted = res["formatted"].as_str().unwrap();
        assert!(formatted.contains("const a"));
        assert!(formatted.contains("function foo"));
    }

    #[tokio::test]
    async fn test_js_format_syntax_error() {
        let tool = JsFormatTool;
        let args = json!({
            "code": "const a = ;",
            "file_path": "test.js"
        });
        
        let res = tool.call(&args).await.unwrap();
        assert_eq!(res["status"], "error");
        assert!(!res["errors"].as_str().unwrap().is_empty());
    }
}
