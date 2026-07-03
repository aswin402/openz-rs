use crate::ir::{Document, Chunk, Paragraph};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChunkingStrategy {
    Fixed,
    Heading,
    Recursive,
    Page,
}

impl std::str::FromStr for ChunkingStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fixed" => Ok(ChunkingStrategy::Fixed),
            "heading" => Ok(ChunkingStrategy::Heading),
            "recursive" => Ok(ChunkingStrategy::Recursive),
            "page" => Ok(ChunkingStrategy::Page),
            _ => Err(format!("Unknown chunking strategy: {}", s)),
        }
    }
}

/// Estimates token count based on standard English text heuristics (4 chars = 1 token).
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4 + 1
}

/// Splits text recursively by character boundaries (paragraphs, sentences, words).
pub fn recursive_split(
    text: &str,
    max_tokens: usize,
    overlap: usize,
    heading: &str,
    chunks: &mut Vec<Chunk>,
) {
    if estimate_tokens(text) <= max_tokens {
        chunks.push(Chunk {
            text: text.to_string(),
            heading: heading.to_string(),
            index: chunks.len(),
        });
        return;
    }

    // Try splitting by double newline, newline, sentences, then spaces.
    let split_seps = ["\n\n", "\n", ". ", "? ", "! ", " "];
    let mut split_done = false;

    for sep in &split_seps {
        let parts: Vec<&str> = text.split(sep).collect();
        if parts.len() > 1 {
            let mut current_chunk = String::new();
            let mut current_tokens = 0;

            for part in parts {
                let part_text = if current_chunk.is_empty() {
                    part.to_string()
                } else {
                    format!("{}{}", sep, part)
                };

                let part_tokens = estimate_tokens(&part_text);
                if current_tokens + part_tokens > max_tokens && !current_chunk.is_empty() {
                    chunks.push(Chunk {
                        text: current_chunk.clone(),
                        heading: heading.to_string(),
                        index: chunks.len(),
                    });

                    // Keep overlap text
                    let words: Vec<&str> = current_chunk.split_whitespace().collect();
                    let overlap_count = words.len().min(overlap);
                    current_chunk = words[words.len() - overlap_count..].join(" ");
                    current_tokens = estimate_tokens(&current_chunk);
                }

                if current_chunk.is_empty() {
                    current_chunk.push_str(part);
                } else {
                    current_chunk.push_str(sep);
                    current_chunk.push_str(part);
                }
                current_tokens += part_tokens;
            }

            if !current_chunk.is_empty() {
                chunks.push(Chunk {
                    text: current_chunk,
                    heading: heading.to_string(),
                    index: chunks.len(),
                });
            }
            split_done = true;
            break;
        }
    }

    // Fallback: if we can't split by separators, hard split by character slices
    if !split_done {
        let char_limit = max_tokens * 4;
        let chars = text.chars().collect::<Vec<char>>();
        let mut pos = 0;
        while pos < chars.len() {
            let end = (pos + char_limit).min(chars.len());
            let chunk_str: String = chars[pos..end].iter().collect();
            chunks.push(Chunk {
                text: chunk_str,
                heading: heading.to_string(),
                index: chunks.len(),
            });
            pos += char_limit - (overlap * 4).min(char_limit - 4).max(4);
        }
    }
}

/// Run the selected chunking strategy on the Document IR.
pub fn chunk_document(
    doc: &Document,
    strategy: ChunkingStrategy,
    max_tokens: usize,
    overlap: usize,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();

    match strategy {
        ChunkingStrategy::Fixed => {
            let mut current_chunk = String::new();
            let mut current_tokens = 0;
            let mut current_heading = String::new();
            let mut p_indices: Vec<usize> = Vec::new();

            let mut i = 0;
            while i < doc.paragraphs.len() {
                let p = &doc.paragraphs[i];
                if p.is_heading {
                    current_heading = p.text.clone();
                }

                let p_tokens = estimate_tokens(&p.text) + 1;
                if current_tokens + p_tokens > max_tokens && !current_chunk.is_empty() {
                    chunks.push(Chunk {
                        text: current_chunk.trim_end().to_string(),
                        heading: current_heading.clone(),
                        index: chunks.len(),
                    });

                    // Backtrack to implement paragraph overlap
                    if overlap > 0 && !p_indices.is_empty() {
                        let mut back_tokens = 0;
                        let mut back_idx = p_indices.len();
                        while back_idx > 0 && back_tokens < overlap {
                            back_idx -= 1;
                            let p_back: &Paragraph = &doc.paragraphs[p_indices[back_idx]];
                            back_tokens += estimate_tokens(&p_back.text) + 1;
                        }
                        i = p_indices[back_idx] + 1;
                    } else {
                        i += 1;
                    }

                    current_chunk.clear();
                    current_tokens = 0;
                    p_indices.clear();
                    continue;
                }

                current_chunk.push_str(&p.text);
                current_chunk.push('\n');
                current_tokens += p_tokens;
                p_indices.push(i);
                i += 1;
            }

            if !current_chunk.is_empty() {
                chunks.push(Chunk {
                    text: current_chunk.trim_end().to_string(),
                    heading: current_heading,
                    index: chunks.len(),
                });
            }
        }
        ChunkingStrategy::Heading => {
            let mut current_section = String::new();
            let mut current_heading = "Header".to_string();

            for p in &doc.paragraphs {
                if p.is_heading {
                    if !current_section.is_empty() {
                        recursive_split(&current_section, max_tokens, overlap, &current_heading, &mut chunks);
                        current_section.clear();
                    }
                    current_heading = p.text.clone();
                }
                current_section.push_str(&p.text);
                current_section.push('\n');
            }

            if !current_section.is_empty() {
                recursive_split(&current_section, max_tokens, overlap, &current_heading, &mut chunks);
            }
        }
        ChunkingStrategy::Recursive => {
            let mut full_text = String::new();
            for p in &doc.paragraphs {
                full_text.push_str(&p.text);
                full_text.push('\n');
            }
            recursive_split(&full_text, max_tokens, overlap, "Document", &mut chunks);
        }
        ChunkingStrategy::Page => {
            if !doc.sections.is_empty() {
                for s in &doc.sections {
                    let mut section_text = String::new();
                    section_text.push_str(&format!("Section: {}\n", s.title));
                    for p in &s.content {
                        section_text.push_str(&p.text);
                        section_text.push('\n');
                    }
                    if section_text.trim().is_empty() {
                        continue;
                    }
                    recursive_split(&section_text, max_tokens, overlap, &s.title, &mut chunks);
                }
            } else {
                let mut full_text = String::new();
                for p in &doc.paragraphs {
                    full_text.push_str(&p.text);
                    full_text.push('\n');
                }
                recursive_split(&full_text, max_tokens, overlap, "Page", &mut chunks);
            }
        }
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Document, Paragraph, Section};

    #[test]
    fn test_chunk_fixed_strategy() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("Paragraph 1. Hello world."));
        doc.paragraphs.push(Paragraph::new("Paragraph 2. Rust is awesome."));
        doc.paragraphs.push(Paragraph::new("Paragraph 3. MCP servers are neat."));

        // Fixed size strategy, 10 tokens max, no overlap
        let chunks = chunk_document(&doc, ChunkingStrategy::Fixed, 10, 0);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_chunk_heading_strategy() {
        let mut doc = Document::new("txt");
        let mut p1 = Paragraph::new("Introduction");
        p1.is_heading = true;
        doc.paragraphs.push(p1);
        doc.paragraphs.push(Paragraph::new("This is the intro section context."));

        let mut p2 = Paragraph::new("Conclusion");
        p2.is_heading = true;
        doc.paragraphs.push(p2);
        doc.paragraphs.push(Paragraph::new("This is the conclusion context."));

        let chunks = chunk_document(&doc, ChunkingStrategy::Heading, 20, 0);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].heading, "Introduction");
        assert_eq!(chunks[1].heading, "Conclusion");
    }

    #[test]
    fn test_chunk_page_strategy() {
        let mut doc = Document::new("txt");
        let s = Section {
            title: "Slide 1".to_string(),
            level: 1,
            index: 0,
            content: vec![Paragraph::new("Slide content text")],
        };
        doc.sections.push(s);

        let chunks = chunk_document(&doc, ChunkingStrategy::Page, 20, 0);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "Slide 1");
    }
}
