use crate::tools::opendoc::ir::Document;
use serde_json::Value;

/// Simple template engine: replace `{{key}}` placeholders with values
pub fn fill_template(doc: &mut Document, vars: &[(String, String)]) -> usize {
    let mut count = 0;

    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        count += super::replace::transform_doc_strings(doc, true, |text| {
            let new_text = text.replace(&placeholder, value.as_str());
            let changed = usize::from(new_text != text);
            (new_text, changed)
        });
    }

    count
}

/// Enhanced template engine with nested objects, loops, and conditionals.
pub fn fill_template_enhanced(doc: &mut Document, vars: &Value) -> usize {
    super::replace::transform_doc_strings(doc, true, |text| render_template(text, vars))
}

pub(crate) use crate::tools::arguments::resolve_dotted_json_path as resolve_path;

/// Check whether a JSON value is truthy.
fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(_) => true,
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(_) => true,
    }
}

/// Format a JSON value as a string for template insertion.
fn format_value(val: &Value) -> String {
    match val {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value).collect();
            items.join(", ")
        }
        Value::Object(obj) => {
            let items: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            items.join(", ")
        }
    }
}

fn is_ident_or_dot(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Find matching closing tag, handling nesting.
fn find_matching_close_in(
    template: &str,
    block_type: &str,
    name: &str,
    start: usize,
) -> Option<usize> {
    let mut depth = 1u32;
    let mut pos = start;

    let open_pattern = match block_type {
        "if" => "{{#if ".to_string(),
        "each" => "{{#each ".to_string(),
        _ => format!("{{{{#{}", name),
    };

    let close_pattern = match block_type {
        "if" => "{{/if}}".to_string(),
        "each" => "{{/each}}".to_string(),
        _ => format!("{{{{/{}}}}}", name),
    };

    while pos < template.len() {
        let remaining = &template[pos..];
        match (
            remaining.starts_with(&close_pattern),
            remaining.starts_with(&open_pattern),
        ) {
            (true, _) => {
                depth -= 1;
                match depth == 0 {
                    true => return Some(pos),
                    false => pos += close_pattern.len(),
                }
            }
            (false, true) => {
                depth += 1;
                pos += open_pattern.len();
            }
            (false, false) => {
                let search_start = match remaining.starts_with("{{") {
                    true => pos + 2,
                    false => pos,
                };
                match search_start >= template.len() {
                    true => break,
                    false => {}
                }
                match template[search_start..].find("{{") {
                    Some(idx) => pos = search_start + idx,
                    None => break,
                }
            }
        }
    }
    None
}

/// Render a template string with the given variables.
/// Returns (output_string, replacement_count).
fn render_template(template: &str, vars: &Value) -> (String, usize) {
    let mut count = 0;

    // Step 1: Process block sections (innermost first)
    let after_blocks = process_all_blocks(template, vars, &mut count);

    // Step 2: Process remaining variable interpolations
    let after_vars = replace_variables(&after_blocks, vars, &mut count);

    (after_vars, count)
}

fn find_first_open_block(s: &str) -> Option<(usize, usize, &'static str, String)> {
    let mut search_pos = 0usize;
    while search_pos < s.len() {
        let rel = s[search_pos..].find("{{#")?;
        let start = search_pos + rel;
        let after_open_brace = start + 3;
        match s[after_open_brace..].find("}}") {
            Some(close_rel) => {
                let inner = &s[after_open_brace..after_open_brace + close_rel];
                let end = after_open_brace + close_rel + 2;
                let (block_type, name) = match (
                    inner.strip_prefix("if "),
                    inner.strip_prefix("each "),
                ) {
                    (Some(rest), _) => ("if", rest),
                    (None, Some(rest)) => ("each", rest),
                    (None, None) => ("#", inner),
                };
                match is_ident_or_dot(name) {
                    true => return Some((start, end, block_type, name.to_string())),
                    false => search_pos = after_open_brace,
                }
            }
            None => return None,
        }
    }
    None
}

/// Process all block sections ({{#...}}...{{/...}}) in a template string.
fn process_all_blocks(template: &str, vars: &Value, count: &mut usize) -> String {
    let mut result = template.to_string();

    loop {
        let current = result.clone();
        let mut found = false;

        match find_first_open_block(&current) {
            Some((start, after_open, block_type, block_name_clean)) => {
                match find_matching_close_in(&current, block_type, &block_name_clean, after_open) {
                    Some(close_pos) => {
                        let content = &current[after_open..close_pos];
                        let close_len = match block_type {
                            "if" => "{{/if}}".len(),
                            "each" => "{{/each}}".len(),
                            _ => format!("{{{{/{}}}}}", block_name_clean).len(),
                        };
                        let end_pos = close_pos + close_len;

                        let expanded = match block_type {
                            "if" => expand_conditional(&block_name_clean, content, vars, count),
                            "each" => expand_each(&block_name_clean, content, vars, count),
                            _ => expand_section(&block_name_clean, content, vars, count),
                        };

                        result =
                            format!("{}{}{}", &current[..start], expanded, &current[end_pos..]);
                        found = true;
                    }
                    None => {}
                }
            }
            None => {}
        }

        match found {
            true => {}
            false => break,
        }
    }

    result
}

fn render_block_body(body: &str, context: &Value, count: &mut usize) -> String {
    let mut sub_count = 0;
    let processed = process_all_blocks(body, context, &mut sub_count);
    let (line, c) = replace_variables_in_str(&processed, context);
    sub_count += c;
    *count += match sub_count == 0 {
        true => 1,
        false => sub_count,
    };
    line
}

fn build_item_context(item: &Value, fallback_root: Option<&Value>) -> Value {
    match item {
        Value::Object(map) => {
            let mut merged = map.clone();
            merged.insert("this".to_string(), item.clone());
            merged.insert(".".to_string(), item.clone());
            Value::Object(merged)
        }
        _ => {
            let mut merged = serde_json::Map::new();
            match fallback_root {
                Some(Value::Object(map)) => {
                    for (k, v) in map {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                _ => {}
            }
            merged.insert("this".to_string(), item.clone());
            merged.insert(".".to_string(), item.clone());
            Value::Object(merged)
        }
    }
}

/// Expand a conditional block: `{{#if name}}...{{else}}...{{/if}}`
fn expand_conditional(name: &str, content: &str, vars: &Value, count: &mut usize) -> String {
    let resolved = resolve_path(vars, name);
    let condition = resolved.is_some_and(is_truthy);

    let else_marker = "{{else}}";
    let else_pos = content.find(else_marker);

    let body = match (condition, else_pos) {
        (true, Some(pos)) => &content[..pos],
        (true, None) => content,
        (false, Some(pos)) => &content[pos + else_marker.len()..],
        (false, None) => "",
    };

    render_block_body(body, vars, count)
}

/// Expand an each loop: `{{#each list}}...{{/each}}`
fn expand_each(name: &str, content: &str, vars: &Value, count: &mut usize) -> String {
    let mut output = String::new();

    match resolve_path(vars, name) {
        Some(Value::Array(items)) => {
            for item in items {
                let context = build_item_context(item, Some(vars));
                let line = render_block_body(content, &context, count);
                output.push_str(&line);
            }
        }
        _ => {}
    }

    output
}

/// Expand a section block: `{{#name}}...{{/name}}`
fn expand_section(name: &str, content: &str, vars: &Value, count: &mut usize) -> String {
    let mut output = String::new();

    match resolve_path(vars, name) {
        Some(Value::Array(items)) => {
            for item in items {
                let context = build_item_context(item, None);
                let line = render_block_body(content, &context, count);
                output.push_str(&line);
            }
        }
        Some(val) => match is_truthy(val) {
            true => {
                let mut context = build_item_context(val, Some(vars));
                match (&mut context, val.is_object()) {
                    (Value::Object(m), false) => {
                        m.insert(name.to_string(), val.clone());
                    }
                    _ => {}
                }
                let line = render_block_body(content, &context, count);
                output.push_str(&line);
            }
            false => {}
        },
        None => {}
    }

    output
}

/// Replace {{variable}} patterns with resolved values.
fn replace_variables(text: &str, vars: &Value, count: &mut usize) -> String {
    let (res, c) = replace_variables_in_str(text, vars);
    *count += c;
    res
}

/// Replace {{variable}} patterns using dot-notation resolution without regex.
fn replace_variables_in_str(text: &str, vars: &Value) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut count = 0usize;
    let mut pos = 0usize;

    while pos < text.len() {
        match text[pos..].find("{{") {
            Some(rel_open) => {
                let open_idx = pos + rel_open;
                out.push_str(&text[pos..open_idx]);
                let after_open = open_idx + 2;
                match text[after_open..].find("}}") {
                    Some(rel_close) => {
                        let close_idx = after_open + rel_close;
                        let candidate = &text[after_open..close_idx];
                        match is_ident_or_dot(candidate) {
                            true => {
                                match resolve_path(vars, candidate) {
                                    Some(val) => {
                                        count += 1;
                                        out.push_str(&format_value(val));
                                    }
                                    None => {
                                        out.push_str(&text[open_idx..close_idx + 2]);
                                    }
                                }
                                pos = close_idx + 2;
                            }
                            false => {
                                out.push_str("{{");
                                pos = after_open;
                            }
                        }
                    }
                    None => {
                        out.push_str(&text[open_idx..]);
                        pos = text.len();
                    }
                }
            }
            None => {
                out.push_str(&text[pos..]);
                break;
            }
        }
    }

    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::opendoc::ir::{Paragraph, Section, Table};

    #[test]
    fn test_fill_single_key() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello {{name}}!"));
        let vars = vec![("name".to_string(), "World".to_string())];
        let count = fill_template(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Hello World!");
    }

    #[test]
    fn test_fill_multiple_vars() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{greeting}} {{name}}!"));
        let vars = vec![
            ("greeting".to_string(), "Hi".to_string()),
            ("name".to_string(), "Alice".to_string()),
        ];
        let count = fill_template(&mut doc, &vars);
        assert_eq!(count, 2);
        assert_eq!(doc.paragraphs[0].text, "Hi Alice!");
    }

    #[test]
    fn test_fill_no_match() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello World"));
        let vars = vec![("foo".to_string(), "bar".to_string())];
        let count = fill_template(&mut doc, &vars);
        assert_eq!(count, 0);
        assert_eq!(doc.paragraphs[0].text, "Hello World");
    }

    #[test]
    fn test_fill_in_section_title() {
        let mut doc = Document::new("txt");
        doc.sections.push(Section {
            title: "Chapter {{num}}".to_string(),
            level: 1,
            index: 0,
            content: vec![],
        });
        let vars = vec![("num".to_string(), "1".to_string())];
        let count = fill_template(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.sections[0].title, "Chapter 1");
    }

    #[test]
    fn test_fill_in_table_cell() {
        let mut doc = Document::new("csv");
        doc.tables.push(Table {
            headers: vec!["Key".to_string()],
            rows: vec![vec!["{{value}}".to_string()]],
            caption: None,
        });
        let vars = vec![("value".to_string(), "42".to_string())];
        let count = fill_template(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.tables[0].rows[0][0], "42");
    }

    #[test]
    fn test_fill_repeated_key() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("{{x}} + {{x}} = {{y}}"));
        let vars = vec![
            ("x".to_string(), "1".to_string()),
            ("y".to_string(), "2".to_string()),
        ];
        let count = fill_template(&mut doc, &vars);
        assert_eq!(count, 2);
        assert_eq!(doc.paragraphs[0].text, "1 + 1 = 2");
    }

    #[test]
    fn test_fill_table_headers_and_caption() {
        let mut doc = Document::new("docx");
        let table = Table {
            headers: vec!["Header {{h}}".to_string()],
            rows: vec![vec!["Value".to_string()]],
            caption: Some("Table Caption {{c}}".to_string()),
        };
        doc.tables.push(table);
        let vars = vec![
            ("h".to_string(), "Alpha".to_string()),
            ("c".to_string(), "Omega".to_string()),
        ];
        let count = fill_template(&mut doc, &vars);
        assert_eq!(count, 2);
        assert_eq!(doc.tables[0].headers[0], "Header Alpha");
        assert_eq!(
            doc.tables[0].caption.as_ref().unwrap(),
            "Table Caption Omega"
        );
    }

    // ═══════════════════════════════════════
    //  Enhanced template tests
    // ═══════════════════════════════════════

    #[test]
    fn test_enhanced_simple_var() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello {{name}}!"));
        let vars = serde_json::json!({"name": "World"});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Hello World!");
    }

    #[test]
    fn test_enhanced_nested_var() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello {{user.name}}!"));
        let vars = serde_json::json!({"user": {"name": "Alice"}});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Hello Alice!");
    }

    #[test]
    fn test_enhanced_deep_nested() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("{{a.b.c.d}}"));
        let vars = serde_json::json!({"a": {"b": {"c": {"d": "deep"}}}});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "deep");
    }

    #[test]
    fn test_enhanced_section_array() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#items}} Item: {{this}}{{/items}}"));
        let vars = serde_json::json!({"items": ["A", "B", "C"]});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 3);
        assert_eq!(doc.paragraphs[0].text, " Item: A Item: B Item: C");
    }

    #[test]
    fn test_enhanced_section_truthy() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#show}}Visible{{/show}}"));
        let vars = serde_json::json!({"show": true});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Visible");
    }

    #[test]
    fn test_enhanced_section_falsy() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#show}}Visible{{/show}}"));
        let vars = serde_json::json!({"show": false});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 0);
        assert_eq!(doc.paragraphs[0].text, "");
    }

    #[test]
    fn test_enhanced_conditional_if_else() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#if show}}Yes{{else}}No{{/if}}"));
        let vars = serde_json::json!({"show": true});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Yes");
    }

    #[test]
    fn test_enhanced_conditional_else_branch() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#if show}}Yes{{else}}No{{/if}}"));
        let vars = serde_json::json!({"show": false});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "No");
    }

    #[test]
    fn test_enhanced_each_loop() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#each users}}User: {{name}}, {{/each}}"));
        let vars = serde_json::json!({
            "users": [
                {"name": "Alice"},
                {"name": "Bob"}
            ]
        });
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 2);
        assert_eq!(doc.paragraphs[0].text, "User: Alice, User: Bob, ");
    }

    #[test]
    fn test_enhanced_nested_block() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new(
            "{{#users}}{{#if active}}{{name}}{{else}}(inactive){{/if}}{{/users}}",
        ));
        let vars = serde_json::json!({
            "users": [
                {"name": "Alice", "active": true},
                {"name": "Bob", "active": false},
                {"name": "Charlie", "active": true}
            ]
        });
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 3);
        assert_eq!(doc.paragraphs[0].text, "Alice(inactive)Charlie");
    }

    #[test]
    fn test_enhanced_table_cell() {
        let mut doc = Document::new("docx");
        let table = Table {
            headers: vec!["{{col1}}".to_string()],
            rows: vec![vec!["{{user.name}}".to_string()]],
            caption: Some("Table: {{title}}".to_string()),
        };
        doc.tables.push(table);
        let vars = serde_json::json!({
            "col1": "Name",
            "user": {"name": "Alice"},
            "title": "Users"
        });
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 3);
        assert_eq!(doc.tables[0].headers[0], "Name");
        assert_eq!(doc.tables[0].rows[0][0], "Alice");
        assert_eq!(doc.tables[0].caption.as_ref().unwrap(), "Table: Users");
    }

    #[test]
    fn test_enhanced_unknown_var_kept() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello {{unknown}}!"));
        let vars = serde_json::json!({"name": "World"});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 0); // {{unknown}} not resolved, not counted
        assert_eq!(doc.paragraphs[0].text, "Hello {{unknown}}!");
    }

    #[test]
    fn test_enhanced_section_with_dot_context() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#items}}{{.}}{{/items}}"));
        let vars = serde_json::json!({"items": ["X", "Y"]});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 2);
        assert_eq!(doc.paragraphs[0].text, "XY");
    }

    #[test]
    fn test_enhanced_section_empty_array() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("{{#items}}Item{{/items}}"));
        let vars = serde_json::json!({"items": []});
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 0);
        assert_eq!(doc.paragraphs[0].text, "");
    }

    #[test]
    fn test_enhanced_mixed_simple_and_nested() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("Report: {{title}}, Author: {{author.name}}"));
        let vars = serde_json::json!({
            "title": "Q4 Summary",
            "author": {"name": "Alice"}
        });
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 2);
        assert_eq!(doc.paragraphs[0].text, "Report: Q4 Summary, Author: Alice");
    }
}
