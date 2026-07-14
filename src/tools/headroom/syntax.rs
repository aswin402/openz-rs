pub fn compress_code_with_options(
    raw_code: &str,
    signatures_only: bool,
    extension: &str,
) -> String {
    let base = if signatures_only {
        extract_signatures(raw_code, extension)
    } else {
        raw_code.to_string()
    };
    crate::agent::context_compactor::compress_code(&base)
}

pub fn extract_signatures(input: &str, extension: &str) -> String {
    let ext = extension.trim_start_matches('.').to_lowercase();
    if ext == "py" || ext == "python" {
        extract_python_signatures(input)
    } else {
        extract_brace_signatures(input)
    }
}

fn extract_python_signatures(input: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            result.push_str(line);
            result.push('\n');
            i += 1;
            continue;
        }
        if trimmed.starts_with("def ") || trimmed.starts_with("class ") || trimmed.starts_with('@')
        {
            let indent = line.len() - trimmed.len();
            result.push_str(line);
            result.push('\n');
            i += 1;
            while i < lines.len() {
                let next = lines[i];
                let next_trimmed = next.trim_start();
                if next_trimmed.is_empty() {
                    i += 1;
                    continue;
                }
                let next_indent = next.len() - next_trimmed.len();
                if next_indent <= indent {
                    break;
                }
                if next_trimmed.starts_with("def ")
                    || next_trimmed.starts_with("class ")
                    || next_trimmed.starts_with('@')
                {
                    result.push_str(next);
                    result.push('\n');
                }
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    result.trim().to_string()
}

fn extract_brace_signatures(input: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut last_word = String::new();
    let mut pending_function = false;

    while i < chars.len() {
        let c = chars[i];
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    out.push(chars[i]);
                } else if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            last_word.push(c);
        } else if !last_word.is_empty() {
            if matches!(last_word.as_str(), "fn" | "function") {
                pending_function = true;
            }
            if c == ';' || c == '}' {
                pending_function = false;
            }
            last_word.clear();
        }

        if c == '{' && pending_function {
            out.push_str("{ ... }");
            pending_function = false;
            i += 1;
            let mut depth = 1usize;
            while i < chars.len() && depth > 0 {
                match chars[i] {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    '"' | '\'' => {
                        let quote = chars[i];
                        i += 1;
                        while i < chars.len() {
                            if chars[i] == '\\' && i + 1 < chars.len() {
                                i += 2;
                                continue;
                            }
                            if chars[i] == quote {
                                break;
                            }
                            i += 1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            continue;
        }

        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}
