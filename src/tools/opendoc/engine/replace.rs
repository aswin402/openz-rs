use crate::tools::opendoc::ir::Document;

#[derive(Debug, Clone)]
enum Atom {
    Digit,
    Word,
    Space,
    Any,
    Literal(char),
    CharClass { negated: bool, chars: Vec<char> },
}

#[derive(Debug, Clone, Copy)]
struct Quantifier {
    min: usize,
    max: Option<usize>,
}

#[derive(Debug, Clone)]
struct Token {
    atom: Atom,
    quant: Quantifier,
}

impl Atom {
    fn matches(&self, c: char) -> bool {
        match self {
            Atom::Digit => c.is_ascii_digit(),
            Atom::Word => c.is_ascii_alphanumeric() || c == '_',
            Atom::Space => c.is_whitespace(),
            Atom::Any => c != '\n',
            Atom::Literal(expected) => c == *expected,
            Atom::CharClass { negated, chars } => chars.contains(&c) ^ *negated,
        }
    }
}

fn parse_quantifier(chars: &[char], idx: usize) -> Result<(Quantifier, usize), String> {
    match chars.get(idx).copied() {
        Some('+') => Ok((Quantifier { min: 1, max: None }, idx + 1)),
        Some('*') => Ok((Quantifier { min: 0, max: None }, idx + 1)),
        Some('?') => Ok((
            Quantifier {
                min: 0,
                max: Some(1),
            },
            idx + 1,
        )),
        Some('{') => {
            let close_rel = chars[idx + 1..]
                .iter()
                .position(|&c| c == '}')
                .ok_or_else(|| "unclosed repetition quantifier".to_string())?;
            let inside: String = chars[idx + 1..idx + 1 + close_rel].iter().collect();
            let next_idx = idx + 1 + close_rel + 1;
            match inside.split_once(',') {
                Some((min_s, max_s)) => {
                    let min = min_s
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "invalid repetition min".to_string())?;
                    let max = match max_s.trim().is_empty() {
                        true => None,
                        false => Some(
                            max_s
                                .trim()
                                .parse::<usize>()
                                .map_err(|_| "invalid repetition max".to_string())?,
                        ),
                    };
                    Ok((Quantifier { min, max }, next_idx))
                }
                None => {
                    let exact = inside
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "invalid repetition count".to_string())?;
                    Ok((
                        Quantifier {
                            min: exact,
                            max: Some(exact),
                        },
                        next_idx,
                    ))
                }
            }
        }
        _ => Ok((
            Quantifier {
                min: 1,
                max: Some(1),
            },
            idx,
        )),
    }
}

fn parse_char_class(chars: &[char], start: usize) -> Result<(Atom, usize), String> {
    let mut i = start;
    let negated = match chars.get(i).copied() {
        Some('^') => {
            i += 1;
            true
        }
        _ => false,
    };
    let mut allowed = Vec::new();
    while i < chars.len() {
        match chars[i] {
            ']' => {
                return Ok((
                    Atom::CharClass {
                        negated,
                        chars: allowed,
                    },
                    i + 1,
                ))
            }
            '\\' => {
                i += 1;
                let esc = *chars
                    .get(i)
                    .ok_or_else(|| "trailing escape in class".to_string())?;
                allowed.push(esc);
                i += 1;
            }
            c => {
                match (chars.get(i + 1).copied(), chars.get(i + 2).copied()) {
                    (Some('-'), Some(end_c)) => match end_c != ']' {
                        true => {
                            for ch in c..=end_c {
                                allowed.push(ch);
                            }
                            i += 3;
                        }
                        false => {
                            allowed.push(c);
                            i += 1;
                        }
                    },
                    _ => {
                        allowed.push(c);
                        i += 1;
                    }
                }
            }
        }
    }
    Err("unclosed character class".to_string())
}

fn parse_pattern(pattern: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (atom, next_i) = match chars[i] {
            '\\' => {
                i += 1;
                let esc = *chars.get(i).ok_or_else(|| "trailing backslash".to_string())?;
                match esc {
                    'd' => (Atom::Digit, i + 1),
                    'w' => (Atom::Word, i + 1),
                    's' => (Atom::Space, i + 1),
                    'b' => {
                        i += 1;
                        continue;
                    }
                    other => (Atom::Literal(other), i + 1),
                }
            }
            '[' => parse_char_class(&chars, i + 1)?,
            ']' => return Err("unmatched closing bracket".to_string()),
            '(' | ')' => {
                i += 1;
                continue;
            }
            '.' => (Atom::Any, i + 1),
            c => (Atom::Literal(c), i + 1),
        };
        let (quant, after_quant) = parse_quantifier(&chars, next_i)?;
        tokens.push(Token { atom, quant });
        i = after_quant;
    }
    Ok(tokens)
}

fn match_tokens_from(tokens: &[Token], text: &[char], pos: usize) -> Option<usize> {
    let first = match tokens.first() {
        Some(t) => t,
        None => return Some(pos),
    };
    let rest = &tokens[1..];
    let mut count = 0usize;
    let max_allowed = first.quant.max.unwrap_or(text.len().saturating_sub(pos));
    while count < max_allowed && pos + count < text.len() && first.atom.matches(text[pos + count]) {
        count += 1;
    }
    match count >= first.quant.min {
        false => None,
        true => {
            let mut try_count = count;
            loop {
                match match_tokens_from(rest, text, pos + try_count) {
                    Some(end_pos) => return Some(end_pos),
                    None => match try_count > first.quant.min {
                        true => try_count -= 1,
                        false => return None,
                    },
                }
            }
        }
    }
}

pub(crate) fn is_pattern_match(pattern: &str, text: &str) -> Result<bool, String> {
    let tokens = parse_pattern(pattern)?;
    match tokens.is_empty() {
        true => return Ok(true),
        false => {}
    }
    let chars: Vec<char> = text.chars().collect();
    Ok((0..=chars.len()).any(|start| match_tokens_from(&tokens, &chars, start).is_some()))
}

pub(crate) fn replace_pattern_all(
    pattern: &str,
    text: &str,
    replacement: &str,
) -> Result<(String, usize), String> {
    let tokens = parse_pattern(pattern)?;
    match tokens.is_empty() {
        true => return Ok((text.to_string(), 0)),
        false => {}
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut pos = 0usize;
    let mut matches = 0usize;
    while pos < chars.len() {
        match match_tokens_from(&tokens, &chars, pos) {
            Some(end_pos) => match end_pos > pos {
                true => {
                    out.push_str(replacement);
                    matches += 1;
                    pos = end_pos;
                }
                false => {
                    out.push(chars[pos]);
                    pos += 1;
                }
            },
            None => {
                out.push(chars[pos]);
                pos += 1;
            }
        }
    }
    Ok((out, matches))
}

pub(crate) fn transform_doc_strings<F>(doc: &mut Document, include_caption: bool, mut f: F) -> usize
where
    F: FnMut(&str) -> (String, usize),
{
    let mut count = 0;

    for p in &mut doc.paragraphs {
        let (new_text, c) = f(&p.text);
        match new_text != p.text {
            true => {
                count += c;
                p.text = new_text;
            }
            false => {}
        }
    }

    for section in &mut doc.sections {
        let (new_title, c) = f(&section.title);
        match new_title != section.title {
            true => {
                count += c;
                section.title = new_title;
            }
            false => {}
        }
    }

    for table in &mut doc.tables {
        match (include_caption, &mut table.caption) {
            (true, Some(cap)) => {
                let (new_cap, c) = f(cap);
                match new_cap != *cap {
                    true => {
                        count += c;
                        *cap = new_cap;
                    }
                    false => {}
                }
            }
            _ => {}
        }
        for header in &mut table.headers {
            let (new_hdr, c) = f(header);
            match new_hdr != *header {
                true => {
                    count += c;
                    *header = new_hdr;
                }
                false => {}
            }
        }
        for row in &mut table.rows {
            for cell in row {
                let (new_cell, c) = f(cell);
                match new_cell != *cell {
                    true => {
                        count += c;
                        *cell = new_cell;
                    }
                    false => {}
                }
            }
        }
    }

    count
}

/// Replace text in a document
pub fn replace_text(doc: &mut Document, find: &str, replace: &str) -> usize {
    match parse_pattern(find).is_ok() {
        false => return 0,
        true => {}
    }

    transform_doc_strings(doc, false, |text| {
        match replace_pattern_all(find, text, replace) {
            Ok((new_text, _)) => {
                let changed = usize::from(new_text != text);
                (new_text, changed)
            }
            Err(_) => (text.to_string(), 0),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::opendoc::ir::{Paragraph, Section, Table};

    #[test]
    fn test_replace_plain_text() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello World"));
        let count = replace_text(&mut doc, "World", "Rust");
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Hello Rust");
    }

    #[test]
    fn test_replace_regex() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello 123 World 456"));
        let count = replace_text(&mut doc, r"\d+", "NUM");
        assert_eq!(count, 1);
        assert_eq!(doc.paragraphs[0].text, "Hello NUM World NUM");
    }

    #[test]
    fn test_replace_multiple_paragraphs() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("foo bar"));
        doc.paragraphs.push(Paragraph::new("baz bar"));
        let count = replace_text(&mut doc, "bar", "qux");
        assert_eq!(count, 2);
        assert_eq!(doc.paragraphs[0].text, "foo qux");
        assert_eq!(doc.paragraphs[1].text, "baz qux");
    }

    #[test]
    fn test_replace_no_match() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello World"));
        let count = replace_text(&mut doc, "Nonexistent", "Rust");
        assert_eq!(count, 0);
        assert_eq!(doc.paragraphs[0].text, "Hello World");
    }

    #[test]
    fn test_replace_in_section_title() {
        let mut doc = Document::new("txt");
        doc.sections.push(Section {
            title: "Introduction".to_string(),
            level: 1,
            index: 0,
            content: vec![],
        });
        let count = replace_text(&mut doc, "Intro", "Conclusion");
        assert_eq!(count, 1);
        assert_eq!(doc.sections[0].title, "Conclusionduction");
    }

    #[test]
    fn test_replace_in_table() {
        let mut doc = Document::new("csv");
        doc.tables.push(Table {
            headers: vec!["Name".to_string(), "City".to_string()],
            rows: vec![vec!["Alice".to_string(), "NYC".to_string()]],
            caption: None,
        });
        let count = replace_text(&mut doc, "NYC", "LA");
        assert_eq!(count, 1);
        assert_eq!(doc.tables[0].rows[0][1], "LA");
    }

    #[test]
    fn test_replace_invalid_regex_returns_zero() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Hello"));
        let count = replace_text(&mut doc, "[invalid", "x");
        assert_eq!(count, 0);
    }
}
