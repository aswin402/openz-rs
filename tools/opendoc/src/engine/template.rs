use crate::ir::Document;
use serde_json::Value;
use regex::Regex;

/// Simple template engine: replace `{{key}}` placeholders with values
pub fn fill_template(doc: &mut Document, vars: &[(String, String)]) -> usize {
    let mut count = 0;

    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        let pattern = regex::escape(&placeholder);
        let re = match regex::RegexBuilder::new(&pattern).size_limit(1_000_000).build() {
            Ok(r) => r,
            Err(_) => continue,
        };

        for p in &mut doc.paragraphs {
            let new = re.replace_all(&p.text, value.as_str()).to_string();
            if new != p.text {
                count += 1;
                p.text = new;
            }
        }

        for section in &mut doc.sections {
            let new = re.replace_all(&section.title, value.as_str()).to_string();
            if new != section.title {
                count += 1;
                section.title = new;
            }
        }

        for table in &mut doc.tables {
            if let Some(ref mut cap) = table.caption {
                let new = re.replace_all(cap, value.as_str()).to_string();
                if new != *cap {
                    count += 1;
                    *cap = new;
                }
            }
            for header in &mut table.headers {
                let new = re.replace_all(header, value.as_str()).to_string();
                if new != *header {
                    count += 1;
                    *header = new;
                }
            }
            for row in &mut table.rows {
                for cell in row {
                    let new = re.replace_all(cell, value.as_str()).to_string();
                    if new != *cell {
                        count += 1;
                        *cell = new;
                    }
                }
            }
        }
    }

    count
}

/// Enhanced template engine with nested objects, loops, and conditionals.
///
/// # Syntax
///
/// | Pattern | Description |
/// |---------|-------------|
/// | `{{variable}}` | Simple variable substitution |
/// | `{{nested.key.path}}` | Dot-notation nested object access |
/// | `{{this}}` or `{{.}}` | Current context reference (inside loops) |
/// | `{{#section}}...{{/section}}` | Section: iterate if array, show once if truthy |
/// | `{{^section}}...{{/section}}` | Inverted section: show if falsy/empty |
/// | `{{#if cond}}...{{else}}...{{/if}}` | Conditional with optional else |
/// | `{{#each list}}...{{/each}}` | Explicit loop context |
///
/// # Example
///
/// ```ignore
/// let vars = serde_json::json!({
///     "title": "Report",
///     "author": { "name": "Alice" },
///     "items": ["Foo", "Bar"],
///     "show_footer": true
/// });
/// fill_template_enhanced(&mut doc, &vars);
/// ```
pub fn fill_template_enhanced(doc: &mut Document, vars: &Value) -> usize {
    let mut count = 0;

    for p in &mut doc.paragraphs {
        let (new, c) = render_template(&p.text, vars);
        if new != p.text {
            count += c;
            p.text = new;
        }
    }

    for section in &mut doc.sections {
        let (new, c) = render_template(&section.title, vars);
        if new != section.title {
            count += c;
            section.title = new;
        }
    }

    for table in &mut doc.tables {
        if let Some(ref mut cap) = table.caption {
            let (new, c) = render_template(cap, vars);
            if new != *cap {
                count += c;
                *cap = new;
            }
        }
        for header in &mut table.headers {
            let (new, c) = render_template(header, vars);
            if new != *header {
                count += c;
                *header = new;
            }
        }
        for row in &mut table.rows {
            for cell in row {
                let (new, c) = render_template(cell, vars);
                if new != *cell {
                    count += c;
                    *cell = new;
                }
            }
        }
    }

    count
}

/// Resolve a dotted path against a JSON value.
fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() || path == "this" || path == "." {
        if let Value::Object(map) = root {
            if let Some(val) = map.get(path) {
                return Some(val);
            }
        }
        return Some(root);
    }
    let mut current = root;
    for segment in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map.get(segment)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Check if a JSON value is "truthy" (for conditionals).
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

/// Find matching closing tag, handling nesting.
fn find_matching_close_in(template: &str, block_type: &str, name: &str, start: usize) -> Option<usize> {
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
        if remaining.starts_with(&close_pattern) {
            depth -= 1;
            if depth == 0 {
                return Some(pos);
            }
            pos += close_pattern.len();
        } else if remaining.starts_with(&open_pattern) {
            depth += 1;
            pos += open_pattern.len();
        } else {
            let search_start = if remaining.starts_with("{{") {
                pos + 2
            } else {
                pos
            };
            if search_start >= template.len() {
                break;
            }
            match template[search_start..].find("{{") {
                Some(idx) => pos = search_start + idx,
                None => break,
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

/// Process all block sections ({{#...}}...{{/...}}) in a template string.
fn process_all_blocks(template: &str, vars: &Value, count: &mut usize) -> String {
    // Regex matches: {{#name}}, {{#if cond}}, {{#each list}}
    let block_re = Regex::new(r"\{\{#(?:if |each )?([\w.]+)\}\}").unwrap();
    let mut result = template.to_string();

    // Process blocks until none remain
    loop {
        let current = result.clone();
        let mut found = false;

        if let Some(caps) = block_re.captures(&current) {
            let cap = caps.get(0).unwrap();
            let full_match = cap.as_str();
            let block_name_clean = caps.get(1).unwrap().as_str();

            let block_type = if full_match.starts_with("{{#if ") {
                "if"
            } else if full_match.starts_with("{{#each ") {
                "each"
            } else {
                "#"
            };

            let start = cap.start();
            let after_open = start + full_match.len();

            if let Some(close_pos) = find_matching_close_in(&current, block_type, block_name_clean, after_open) {
                let content = &current[after_open..close_pos];
                let close_len = match block_type {
                    "if" => "{{/if}}".len(),
                    "each" => "{{/each}}".len(),
                    _ => format!("{{{{/{}}}}}", block_name_clean).len(),
                };
                let end_pos = close_pos + close_len;

                let expanded = match block_type {
                    "if" => expand_conditional(block_name_clean, content, vars, count),
                    "each" => expand_each(block_name_clean, content, vars, count),
                    _ => expand_section(block_name_clean, content, vars, count),
                };

                let new_result = format!("{}{}{}", &current[..start], expanded, &current[end_pos..]);
                result = new_result;
                found = true;
            }
        }

        if !found {
            break;
        }
    }

    result
}

/// Expand a conditional block: {{#if name}}...{{else}}...{{/if}}
fn expand_conditional(name: &str, content: &str, vars: &Value, count: &mut usize) -> String {
    let resolved = resolve_path(vars, name);
    let condition = resolved.is_some_and(is_truthy);

    // Check for {{else}} in content
    let else_marker = "{{else}}";
    let else_pos = content.find(else_marker);

    let body = if condition {
        // Show if-body (before else)
        match else_pos {
            Some(pos) => &content[..pos],
            None => content,
        }
    } else {
        // Show else-body (after else)
        match else_pos {
            Some(pos) => &content[pos + else_marker.len()..],
            None => "",
        }
    };

    // Recursively process any nested blocks in body
    let mut sub_count = 0;
    let expanded = process_all_blocks(body, vars, &mut sub_count);
    // Replace any remaining variables
    let (final_str, c) = replace_variables_in_str(&expanded, vars);
    sub_count += c;

    if sub_count == 0 {
        *count += 1;
    } else {
        *count += sub_count;
    }
    final_str
}

/// Expand an each loop: {{#each list}}...{{/each}}
fn expand_each(name: &str, content: &str, vars: &Value, count: &mut usize) -> String {
    let mut output = String::new();

    if let Some(Value::Array(items)) = resolve_path(vars, name) {
        for item in items {
            // Deep-merge the current item into context for direct access
            let context = if let Value::Object(map) = item {
                let mut merged = map.clone();
                merged.insert("this".to_string(), item.clone());
                merged.insert(".".to_string(), item.clone());
                Value::Object(merged)
            } else {
                let mut merged = serde_json::Map::new();
                if let Value::Object(map) = vars {
                    for (k, v) in map {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                merged.insert("this".to_string(), item.clone());
                merged.insert(".".to_string(), item.clone());
                Value::Object(merged)
            };

            // Recursively process blocks in content
            let mut sub_count = 0;
            let processed = process_all_blocks(content, &context, &mut sub_count);
            let (line, c) = replace_variables_in_str(&processed, &context);
            sub_count += c;

            if sub_count == 0 {
                *count += 1;
            } else {
                *count += sub_count;
            }
            output.push_str(&line);
        }
    }

    output
}

/// Expand a section block: {{#name}}...{{/name}}
/// If name resolves to an array, iterate. If truthy scalar, show once with context.
fn expand_section(name: &str, content: &str, vars: &Value, count: &mut usize) -> String {
    let mut output = String::new();

    if let Some(resolved) = resolve_path(vars, name) {
        match resolved {
            Value::Array(items) => {
                for item in items {
                    let context = if let Value::Object(map) = item {
                        let mut merged = map.clone();
                        merged.insert("this".to_string(), item.clone());
                        merged.insert(".".to_string(), item.clone());
                        Value::Object(merged)
                    } else {
                        Value::Object({
                            let mut m = serde_json::Map::new();
                            m.insert("this".to_string(), item.clone());
                            m.insert(".".to_string(), item.clone());
                            m
                        })
                    };
                    let mut sub_count = 0;
                    let processed = process_all_blocks(content, &context, &mut sub_count);
                    let (line, c) = replace_variables_in_str(&processed, &context);
                    sub_count += c;

                    if sub_count == 0 {
                        *count += 1;
                    } else {
                        *count += sub_count;
                    }
                    output.push_str(&line);
                }
            }
            val if is_truthy(val) => {
                // Set context to the resolved value
                let context = if let Value::Object(map) = val {
                    let mut merged = map.clone();
                    merged.insert("this".to_string(), val.clone());
                    Value::Object(merged)
                } else {
                    // For scalars, merge into root context + override
                    let mut m: serde_json::Map<String, Value> = serde_json::Map::new();
                    if let Value::Object(root_map) = vars {
                        for (k, v) in root_map {
                            m.insert(k.clone(), v.clone());
                        }
                    }
                    m.insert(name.to_string(), val.clone());
                    m.insert("this".to_string(), val.clone());
                    Value::Object(m)
                };
                let mut sub_count = 0;
                let processed = process_all_blocks(content, &context, &mut sub_count);
                let (line, c) = replace_variables_in_str(&processed, &context);
                sub_count += c;

                if sub_count == 0 {
                    *count += 1;
                } else {
                    *count += sub_count;
                }
                output.push_str(&line);
            }
            _ => {} // falsy → show nothing
        }
    }

    output
}

/// Replace {{variable}} patterns with resolved values.
fn replace_variables(text: &str, vars: &Value, count: &mut usize) -> String {
    let (res, c) = replace_variables_in_str(text, vars);
    *count += c;
    res
}

/// Replace {{variable}} patterns using dot-notation resolution.
fn replace_variables_in_str(text: &str, vars: &Value) -> (String, usize) {
    let re = Regex::new(r"\{\{([\w.]+)\}\}").unwrap();
    let mut count = 0;

    let result = re.replace_all(text, |caps: &regex::Captures| {
        let path = &caps[1];
        match resolve_path(vars, path) {
            Some(val) => {
                count += 1;
                format_value(val)
            }
            None => {
                // Keep the original placeholder if not found
                format!("{{{{{}}}}}", path)
            }
        }
    });

    (result.to_string(), count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Paragraph, Section, Table};

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
        doc.paragraphs.push(Paragraph::new("{{greeting}} {{name}}!"));
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
        assert_eq!(doc.tables[0].caption.as_ref().unwrap(), "Table Caption Omega");
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
        doc.paragraphs.push(Paragraph::new(
            "{{#each users}}User: {{name}}, {{/each}}",
        ));
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
        assert_eq!(
            doc.tables[0].caption.as_ref().unwrap(),
            "Table: Users"
        );
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
        doc.paragraphs.push(Paragraph::new(
            "Report: {{title}}, Author: {{author.name}}",
        ));
        let vars = serde_json::json!({
            "title": "Q4 Summary",
            "author": {"name": "Alice"}
        });
        let count = fill_template_enhanced(&mut doc, &vars);
        assert_eq!(count, 2);
        assert_eq!(
            doc.paragraphs[0].text,
            "Report: Q4 Summary, Author: Alice"
        );
    }
}
