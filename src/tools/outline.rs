use crate::tools::Tool;
use anyhow::{anyhow, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;
use serde_json::{json, Value};
use std::fs;

pub struct CodeOutlineTool;

#[derive(serde::Serialize)]
struct Symbol {
    line: usize,
    kind: String,
    name: String,
    definition: String,
}

pub(crate) fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

pub(crate) fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

pub(crate) fn strip_keyword_with_space<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
    input.strip_prefix(keyword).and_then(|rest| {
        rest.chars()
            .next()
            .filter(|ch| ch.is_whitespace())
            .map(|_| rest.trim_start())
    })
}

fn take_ident(input: &str, allow_angle_brackets: bool) -> Option<&str> {
    let len = input
        .char_indices()
        .take_while(|(_, ch)| {
            is_ident_continue(*ch) || (allow_angle_brackets && matches!(*ch, '<' | '>'))
        })
        .map(|(idx, ch)| idx + ch.len_utf8())
        .last()?;
    Some(&input[..len])
}

fn parse_rust_outline_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    let after_pub = strip_keyword_with_space(trimmed, "pub").unwrap_or(trimmed);
    ["fn", "struct", "enum", "trait", "impl", "type"]
        .into_iter()
        .find_map(|kw| {
            let rest = strip_keyword_with_space(after_pub, kw)?;
            let name = take_ident(rest, true)?;
            Some((kw.to_string(), name.to_string()))
        })
}

fn parse_python_outline_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    ["def", "class"].into_iter().find_map(|kw| {
        let rest = strip_keyword_with_space(trimmed, kw)?;
        let name = take_ident(rest, false)?;
        Some((kw.to_string(), name.to_string()))
    })
}

fn parse_go_outline_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    ["func", "type"].into_iter().find_map(|kw| {
        let rest = strip_keyword_with_space(trimmed, kw)?;
        let after_receiver = rest
            .strip_prefix('(')
            .and_then(|inner| inner.split_once(')'))
            .and_then(|(_, tail)| {
                tail.chars()
                    .next()
                    .filter(|ch| ch.is_whitespace())
                    .map(|_| tail.trim_start())
            })
            .unwrap_or(rest);
        let name = take_ident(after_receiver, false)?;
        Some((kw.to_string(), name.to_string()))
    })
}

fn parse_fallback_outline_line(trimmed: &str) -> Option<(String, String)> {
    let has_decl = ["fn ", "def ", "function ", "class ", "struct "]
        .iter()
        .any(|needle| trimmed.contains(needle));
    match has_decl {
        true => Some((
            "unknown".to_string(),
            trimmed.split_whitespace().nth(1).unwrap_or("").to_string(),
        )),
        false => None,
    }
}

fn is_comment_line(ext: &str, trimmed: &str) -> bool {
    let hash_comment = matches!(ext, "py" | "rb" | "sh") && trimmed.starts_with('#');
    trimmed.starts_with("//") || trimmed.starts_with("/*") || hash_comment
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
            .or_else(|| arguments.get("filePath"))
            .or_else(|| arguments.get("path"))
            .or_else(|| arguments.get("file"))
            .or_else(|| arguments.get("target_file"))
            .or_else(|| arguments.get("targetFile"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Missing 'file_path' parameter"))?;
        let file_path = crate::config::resolve_path(file_path_str);

        match file_path.exists() {
            true => {}
            false => return Err(anyhow!("File '{}' does not exist", file_path_str)),
        }

        let ext = file_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let content = fs::read_to_string(&file_path)?;

        let symbols = match ext.as_str() {
            "js" | "ts" | "jsx" | "tsx" => {
                let allocator = Allocator::default();
                let source_type =
                    SourceType::from_path(&file_path).unwrap_or_else(|_| SourceType::default());
                let parser_res = Parser::new(&allocator, &content, source_type).parse();

                let mut visitor = OutlineVisitor {
                    symbols: Vec::new(),
                    source_text: &content,
                };
                visitor.visit_program(&parser_res.program);
                visitor.symbols
            }
            _ => content
                .lines()
                .enumerate()
                .filter_map(|(idx, line)| {
                    let line_num = idx + 1;
                    let trimmed = line.trim();
                    match trimmed.is_empty() || is_comment_line(&ext, trimmed) {
                        true => None,
                        false => {
                            let parsed = match ext.as_str() {
                                "rs" => parse_rust_outline_line(line),
                                "py" => parse_python_outline_line(line),
                                "go" => parse_go_outline_line(line),
                                _ => parse_fallback_outline_line(trimmed),
                            };
                            parsed.map(|(kind, name)| Symbol {
                                line: line_num,
                                kind,
                                name,
                                definition: trimmed.to_string(),
                            })
                        }
                    }
                })
                .collect(),
        };

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

impl<'a> OutlineVisitor<'a> {
    fn push_span_symbol(&mut self, start_u32: u32, end_u32: u32, kind: &str, name: &str) {
        let start = start_u32 as usize;
        let end = end_u32 as usize;
        let definition = self
            .source_text
            .get(start..end)
            .map(|s| s.lines().next().unwrap_or("").trim().to_string())
            .unwrap_or_else(|| format!("{} {}", kind, name));

        let line_num = self
            .source_text
            .get(..start)
            .map(|prefix| prefix.lines().count() + 1)
            .unwrap_or(0);

        self.symbols.push(Symbol {
            line: line_num,
            kind: kind.to_string(),
            name: name.to_string(),
            definition,
        });
    }
}

impl<'a> Visit<'a> for OutlineVisitor<'a> {
    fn visit_function(&mut self, func: &Function<'a>, flags: ScopeFlags) {
        match &func.id {
            Some(ident) => {
                self.push_span_symbol(func.span.start, func.span.end, "function", &ident.name)
            }
            None => {}
        }
        oxc_ast::visit::walk::walk_function(self, func, flags);
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        match &class.id {
            Some(ident) => {
                self.push_span_symbol(class.span.start, class.span.end, "class", &ident.name)
            }
            None => {}
        }
        oxc_ast::visit::walk::walk_class(self, class);
    }

    fn visit_ts_interface_declaration(&mut self, decl: &TSInterfaceDeclaration<'a>) {
        self.push_span_symbol(decl.span.start, decl.span.end, "interface", &decl.id.name);
        oxc_ast::visit::walk::walk_ts_interface_declaration(self, decl);
    }

    fn visit_ts_type_alias_declaration(&mut self, decl: &TSTypeAliasDeclaration<'a>) {
        self.push_span_symbol(decl.span.start, decl.span.end, "type", &decl.id.name);
        oxc_ast::visit::walk::walk_ts_type_alias_declaration(self, decl);
    }
}

#[cfg(test)]
#[path = "outline_tests.rs"]
mod tests;

