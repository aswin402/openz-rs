pub fn compress_code_with_options(
    raw_code: &str,
    signatures_only: bool,
    extension: &str,
) -> String {
    let base = match signatures_only {
        true => extract_signatures(raw_code, extension),
        false => raw_code.to_string(),
    };
    crate::agent::context_compactor::compress_code(&base)
}

pub fn extract_signatures(input: &str, extension: &str) -> String {
    let ext = extension.trim_start_matches('.').to_lowercase();
    match matches!(ext.as_str(), "py" | "python") {
        true => extract_python_signatures(input),
        false => extract_brace_signatures(input),
    }
}

fn is_py_import(trimmed: &str) -> bool {
    trimmed.starts_with("import ") || trimmed.starts_with("from ")
}

fn is_py_signature(trimmed: &str) -> bool {
    trimmed.starts_with("def ") || trimmed.starts_with("class ") || trimmed.starts_with('@')
}

fn extract_python_signatures(input: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        match is_py_import(trimmed) {
            true => {
                result.push_str(line);
                result.push('\n');
                i += 1;
                continue;
            }
            false => {}
        }
        match is_py_signature(trimmed) {
            true => {
                let indent = line.len() - trimmed.len();
                result.push_str(line);
                result.push('\n');
                i += 1;
                while i < lines.len() {
                    let next = lines[i];
                    let next_trimmed = next.trim_start();
                    match next_trimmed.is_empty() {
                        true => {
                            i += 1;
                            continue;
                        }
                        false => {}
                    }
                    let next_indent = next.len() - next_trimmed.len();
                    match next_indent <= indent {
                        true => break,
                        false => {}
                    }
                    match is_py_signature(next_trimmed) {
                        true => {
                            result.push_str(next);
                            result.push('\n');
                        }
                        false => {}
                    }
                    i += 1;
                }
                continue;
            }
            false => {}
        }
        i += 1;
    }
    result.trim().to_string()
}

fn skip_quoted_in_body(chars: &[char], quote: char, mut i: usize) -> usize {
    while i < chars.len() {
        match chars[i] == '\\' && i + 1 < chars.len() {
            true => {
                i += 2;
                continue;
            }
            false => {}
        }
        match chars[i] == quote {
            true => break,
            false => {}
        }
        i += 1;
    }
    i
}

fn extract_brace_signatures(input: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut last_word = String::new();
    let mut pending_function = false;

    while i < chars.len() {
        let c = chars[i];
        match c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            true => {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            false => {}
        }
        match c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            true => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(chars.len());
                continue;
            }
            false => {}
        }
        match c == '"' || c == '\'' {
            true => {
                let quote = c;
                out.push(c);
                i += 1;
                while i < chars.len() {
                    out.push(chars[i]);
                    match chars[i] == '\\' && i + 1 < chars.len() {
                        true => {
                            i += 1;
                            out.push(chars[i]);
                        }
                        false => match chars[i] == quote {
                            true => {
                                i += 1;
                                break;
                            }
                            false => {}
                        },
                    }
                    i += 1;
                }
                continue;
            }
            false => {}
        }
        match c.is_alphanumeric() || c == '_' {
            true => last_word.push(c),
            false => match !last_word.is_empty() {
                true => {
                    match matches!(last_word.as_str(), "fn" | "function") {
                        true => pending_function = true,
                        false => {}
                    }
                    match c == ';' || c == '}' {
                        true => pending_function = false,
                        false => {}
                    }
                    last_word.clear();
                }
                false => {}
            },
        }

        match c == '{' && pending_function {
            true => {
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
                            i = skip_quoted_in_body(&chars, quote, i + 1);
                        }
                        _ => {}
                    }
                    i += 1;
                }
                continue;
            }
            false => {}
        }

        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}
