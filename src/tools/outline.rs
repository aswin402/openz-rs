use crate::tools::Tool;
use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;

pub struct CodeOutlineTool;

#[derive(serde::Serialize)]
struct Symbol {
    line: usize,
    kind: String,
    name: String,
    definition: String,
}

#[async_trait::async_trait]
impl Tool for CodeOutlineTool {
    fn name(&self) -> &str {
        "code_outline"
    }

    fn description(&self) -> &str {
        "Extract definitions (classes, structs, functions, traits, methods) from a source file to understand its structure without reading the whole file."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the source file to parse (e.g. 'src/main.rs')."
                }
            },
            "required": ["file_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let file_path_str = arguments
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'file_path' parameter"))?;
        let file_path = PathBuf::from(file_path_str);

        if !file_path.exists() {
            return Err(anyhow!("File '{}' does not exist", file_path_str));
        }

        let ext = file_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let content = fs::read_to_string(&file_path)?;
        let mut symbols = Vec::new();

        if ext == "js" || ext == "ts" || ext == "jsx" || ext == "tsx" {
            // High-precision AST parsing using Oxc
            let allocator = Allocator::default();
            let source_type =
                SourceType::from_path(&file_path).unwrap_or_else(|_| SourceType::default());
            let parser_res = Parser::new(&allocator, &content, source_type).parse();

            let mut visitor = OutlineVisitor {
                symbols: Vec::new(),
                source_text: &content,
            };
            visitor.visit_program(&parser_res.program);
            symbols = visitor.symbols;
        } else {
            // Compile regexes for different languages
            let re_rust =
                Regex::new(r"^\s*(pub\s+)?(fn|struct|enum|trait|impl|type)\s+([a-zA-Z0-9_<>]+)")?;
            let re_python = Regex::new(r"^\s*(def|class)\s+([a-zA-Z0-9_]+)")?;
            let re_go = Regex::new(r"^\s*(func|type)\s+(\([^\)]+\)\s+)?([a-zA-Z0-9_]+)")?;

            for (idx, line) in content.lines().enumerate() {
                let line_num = idx + 1;
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                let is_comment = if ext == "py" || ext == "rb" || ext == "sh" {
                    trimmed.starts_with("//")
                        || trimmed.starts_with("#")
                        || trimmed.starts_with("/*")
                } else {
                    trimmed.starts_with("//") || trimmed.starts_with("/*")
                };

                if is_comment {
                    continue;
                }

                match ext.as_str() {
                    "rs" => {
                        if let Some(cap) = re_rust.captures(line) {
                            symbols.push(Symbol {
                                line: line_num,
                                kind: cap.get(2).unwrap().as_str().to_string(),
                                name: cap.get(3).unwrap().as_str().to_string(),
                                definition: trimmed.to_string(),
                            });
                        }
                    }
                    "py" => {
                        if let Some(cap) = re_python.captures(line) {
                            symbols.push(Symbol {
                                line: line_num,
                                kind: cap.get(1).unwrap().as_str().to_string(),
                                name: cap.get(2).unwrap().as_str().to_string(),
                                definition: trimmed.to_string(),
                            });
                        }
                    }
                    "go" => {
                        if let Some(cap) = re_go.captures(line) {
                            symbols.push(Symbol {
                                line: line_num,
                                kind: cap.get(1).unwrap().as_str().to_string(),
                                name: cap.get(3).unwrap().as_str().to_string(),
                                definition: trimmed.to_string(),
                            });
                        }
                    }
                    _ => {
                        // Fallback generic scanner
                        if trimmed.contains("fn ")
                            || trimmed.contains("def ")
                            || trimmed.contains("function ")
                            || trimmed.contains("class ")
                            || trimmed.contains("struct ")
                        {
                            symbols.push(Symbol {
                                line: line_num,
                                kind: "unknown".to_string(),
                                name: trimmed.split_whitespace().nth(1).unwrap_or("").to_string(),
                                definition: trimmed.to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(json!({
            "status": "success",
            "file": file_path_str,
            "symbols": symbols
        }))
    }
}

struct OutlineVisitor<'a> {
    symbols: Vec<Symbol>,
    source_text: &'a str,
}

impl<'a> Visit<'a> for OutlineVisitor<'a> {
    fn visit_function(&mut self, func: &Function<'a>, flags: ScopeFlags) {
        if let Some(ident) = &func.id {
            let start = func.span.start as usize;
            let end = func.span.end as usize;
            let definition = self
                .source_text
                .get(start..end)
                .map(|s| s.lines().next().unwrap_or("").trim().to_string())
                .unwrap_or_else(|| format!("function {}", ident.name));

            // Safe slicing: ensure we don't panic on non-UTF-8 boundaries
            let line_num = if start <= self.source_text.len() {
                self.source_text[..start].lines().count() + 1
            } else {
                0
            };

            self.symbols.push(Symbol {
                line: line_num,
                kind: "function".to_string(),
                name: ident.name.to_string(),
                definition,
            });
        }
        oxc_ast::visit::walk::walk_function(self, func, flags);
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        if let Some(ident) = &class.id {
            let start = class.span.start as usize;
            let end = class.span.end as usize;
            let definition = self
                .source_text
                .get(start..end)
                .map(|s| s.lines().next().unwrap_or("").trim().to_string())
                .unwrap_or_else(|| format!("class {}", ident.name));

            let line_num = if start <= self.source_text.len() {
                self.source_text[..start].lines().count() + 1
            } else {
                0
            };

            self.symbols.push(Symbol {
                line: line_num,
                kind: "class".to_string(),
                name: ident.name.to_string(),
                definition,
            });
        }
        oxc_ast::visit::walk::walk_class(self, class);
    }

    fn visit_ts_interface_declaration(&mut self, decl: &TSInterfaceDeclaration<'a>) {
        let ident = &decl.id;
        let start = decl.span.start as usize;
        let end = decl.span.end as usize;
        let definition = self
            .source_text
            .get(start..end)
            .map(|s| s.lines().next().unwrap_or("").trim().to_string())
            .unwrap_or_else(|| format!("interface {}", ident.name));

        let line_num = if start <= self.source_text.len() {
            self.source_text[..start].lines().count() + 1
        } else {
            0
        };

        self.symbols.push(Symbol {
            line: line_num,
            kind: "interface".to_string(),
            name: ident.name.to_string(),
            definition,
        });
        oxc_ast::visit::walk::walk_ts_interface_declaration(self, decl);
    }

    fn visit_ts_type_alias_declaration(&mut self, decl: &TSTypeAliasDeclaration<'a>) {
        let ident = &decl.id;
        let start = decl.span.start as usize;
        let end = decl.span.end as usize;
        let definition = self
            .source_text
            .get(start..end)
            .map(|s| s.lines().next().unwrap_or("").trim().to_string())
            .unwrap_or_else(|| format!("type {}", ident.name));

        let line_num = if start <= self.source_text.len() {
            self.source_text[..start].lines().count() + 1
        } else {
            0
        };

        self.symbols.push(Symbol {
            line: line_num,
            kind: "type".to_string(),
            name: ident.name.to_string(),
            definition,
        });
        oxc_ast::visit::walk::walk_ts_type_alias_declaration(self, decl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_code_outline() -> Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_outline_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        let rust_file = temp_dir.join("main.rs");
        std::fs::write(
            &rust_file,
            "
            pub fn run_app() {
                println!(\"Hello!\");
            }
            struct Config {
                port: u16,
            }
        ",
        )?;

        let tool = CodeOutlineTool;
        let res = tool
            .call(&json!({
                "file_path": rust_file.to_str().unwrap()
            }))
            .await?;

        assert_eq!(res["status"], "success");
        let symbols = res["symbols"].as_array().unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0]["kind"], "fn");
        assert_eq!(symbols[0]["name"], "run_app");
        assert_eq!(symbols[1]["kind"], "struct");
        assert_eq!(symbols[1]["name"], "Config");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_code_outline_js_ts() -> Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("openz_outline_js_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir)?;

        let ts_file = temp_dir.join("app.ts");
        std::fs::write(
            &ts_file,
            "
            export interface User {
                id: number;
                name: string;
            }
            class UserService {
                getUser(id: number): User {
                    return { id, name: 'OpenZ' };
                }
            }
            function logUser(u: User) {
                console.log(u.name);
            }
        ",
        )?;

        let tool = CodeOutlineTool;
        let res = tool
            .call(&json!({
                "file_path": ts_file.to_str().unwrap()
            }))
            .await?;

        assert_eq!(res["status"], "success");
        let symbols = res["symbols"].as_array().unwrap();

        // Should find interface User, class UserService, and function logUser
        assert!(symbols
            .iter()
            .any(|s| s["kind"] == "interface" && s["name"] == "User"));
        assert!(symbols
            .iter()
            .any(|s| s["kind"] == "class" && s["name"] == "UserService"));
        assert!(symbols
            .iter()
            .any(|s| s["kind"] == "function" && s["name"] == "logUser"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
