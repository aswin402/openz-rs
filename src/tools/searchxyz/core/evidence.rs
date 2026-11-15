use std::collections::HashSet;

use crate::tools::searchxyz::core::extractor::ExtractedContent;

pub fn build_evidence_summary(query: &str, pages: &[ExtractedContent]) -> String {
    let mut output = format!(
        "---\n## Evidence Summary\n- **Query:** {}\n- **Sources Reviewed:** {}\n\n",
        query,
        pages.len()
    );

    push_bullet_section(
        &mut output,
        "### Claims\n",
        &extract_claims(query, pages, 8),
        "- No concise claims extracted from retrieved pages.\n",
    );

    output.push_str("\n### Evidence Sources\n");
    for page in pages {
        output.push_str(&format!("- **{}** -- {}\n", page.title, page.url));
    }

    push_bullet_section(
        &mut output,
        "\n### Potential Conflicts\n",
        &detect_conflicts(pages),
        "- No explicit conflicts detected in retrieved sources.\n",
    );

    push_bullet_section(
        &mut output,
        "\n### Unknowns\n",
        &extract_unknowns(pages),
        "- No explicit unknowns detected; verify missing pricing, dates, and private implementation details before final claims.\n",
    );
    output.push('\n');
    output
}

fn push_bullet_section(output: &mut String, heading: &str, items: &[String], empty_msg: &str) {
    output.push_str(heading);
    match items.is_empty() {
        true => output.push_str(empty_msg),
        false => {
            for item in items {
                output.push_str(&format!("- {}\n", item));
            }
        }
    }
}

fn extract_query_terms(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    query
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::trim)
        .filter(|tok| tok.chars().count() >= 2)
        .map(str::to_ascii_lowercase)
        .filter(|tok| seen.insert(tok.clone()))
        .collect()
}

fn score_claim_sentence(sentence: &str, query_terms: &[String], position_idx: usize) -> i32 {
    let lower = sentence.to_ascii_lowercase();
    let term_hits = query_terms
        .iter()
        .filter(|term| lower.contains(term.as_str()))
        .count() as i32;
    let has_numbers_or_units = sentence
        .chars()
        .any(|c| c.is_ascii_digit() || matches!(c, '$' | '€' | '£' | '%'));
    let early_bonus = (4_i32).saturating_sub(position_idx.min(4) as i32);
    term_hits * 10 + (has_numbers_or_units as i32) * 6 + early_bonus
}

fn extract_claims(query: &str, pages: &[ExtractedContent], limit: usize) -> Vec<String> {
    let query_terms = extract_query_terms(query);
    let mut seen = HashSet::new();
    let mut scored = Vec::new();

    for page in pages {
        for (idx, sentence) in split_sentences(&page.content_markdown).into_iter().enumerate() {
            let trimmed = sentence.trim();
            let valid_len = (24..=240).contains(&trimmed.len());
            let key = trimmed.to_ascii_lowercase();
            match valid_len && !looks_like_boilerplate(trimmed) && seen.insert(key) {
                true => {
                    let score = score_claim_sentence(trimmed, &query_terms, idx);
                    scored.push((score, format!("{trimmed} [{}]", page.url)));
                }
                false => {}
            }
        }
    }

    scored.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, claim)| claim)
        .collect()
}

fn detect_conflicts(pages: &[ExtractedContent]) -> Vec<String> {
    let joined = pages
        .iter()
        .map(|page| page.content_markdown.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    let rules: [(bool, &str); 4] = [
        (
            contains_positive_phrase(&joined, "free tier", &["no free tier", "without free tier"])
                && joined.contains("no free tier"),
            "Sources mention both free tier and no free tier.",
        ),
        (
            joined.contains("open source") && joined.contains("proprietary"),
            "Sources mention both open source and proprietary positioning.",
        ),
        (
            contains_positive_phrase(&joined, "available", &["not available", "unavailable"])
                && (joined.contains("not available") || joined.contains("unavailable")),
            "Sources mention both availability and non-availability.",
        ),
        (
            joined.contains("real-time") && joined.contains("offline"),
            "Sources mix real-time and offline positioning; clarify mode-specific behavior.",
        ),
    ];

    rules
        .into_iter()
        .filter(|(cond, _)| *cond)
        .map(|(_, msg)| msg.to_string())
        .collect()
}

fn contains_positive_phrase(text: &str, phrase: &str, negative_forms: &[&str]) -> bool {
    text.match_indices(phrase).any(|(idx, _)| {
        let start = crate::tools::headroom::floor_char_boundary(text, idx.saturating_sub(16));
        let end = crate::tools::headroom::ceil_char_boundary(
            text,
            (idx + phrase.len() + 16).min(text.len()),
        );
        let window = &text[start..end];
        !negative_forms
            .iter()
            .any(|negative| window.contains(negative))
    })
}

fn extract_unknowns(pages: &[ExtractedContent]) -> Vec<String> {
    let markers = [
        "not publicly disclosed",
        "not disclosed",
        "unknown",
        "unclear",
        "not listed",
        "contact sales",
        "pricing not available",
        "requires login",
    ];

    pages
        .iter()
        .flat_map(|page| {
            split_sentences(&page.content_markdown)
                .into_iter()
                .filter_map(move |sentence| {
                    let lower = sentence.to_ascii_lowercase();
                    markers
                        .iter()
                        .any(|marker| lower.contains(marker))
                        .then(|| format!("{} [{}]", sentence.trim(), page.url))
                })
        })
        .take(6)
        .collect()
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        match matches!(ch, '.' | '!' | '?') {
            true => {
                let sentence = current.trim();
                match sentence.is_empty() {
                    true => {}
                    false => sentences.push(sentence.to_string()),
                }
                current.clear();
            }
            false => {}
        }
    }
    let rest = current.trim();
    match rest.is_empty() {
        true => {}
        false => sentences.push(rest.to_string()),
    }
    sentences
}

fn looks_like_boilerplate(sentence: &str) -> bool {
    let lower = sentence.to_ascii_lowercase();
    lower.contains("cookie")
        || lower.contains("privacy policy")
        || lower.contains("terms of service")
        || lower.contains("sign in")
        || lower.contains("subscribe")
        || lower.contains("all rights reserved")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_summary_contains_claims_sources_conflicts_and_unknowns() {
        let pages = vec![ExtractedContent {
            url: "https://example.com/fugu".to_string(),
            title: "Fugu Pricing".to_string(),
            description: String::new(),
            content_markdown: "Fugu pricing starts at $20/month. No free tier is available. However, routing internals are not publicly disclosed.".to_string(),
            links: vec![],
        }];

        let summary = build_evidence_summary("fugu pricing", &pages);

        assert!(summary.contains("## Evidence Summary"));
        assert!(summary.contains("### Claims"));
        assert!(summary.contains("Fugu pricing starts at $20/month"));
        assert!(summary.contains("### Evidence Sources"));
        assert!(summary.contains("https://example.com/fugu"));
        assert!(summary.contains("### Potential Conflicts"));
        assert!(summary.contains("No explicit conflicts detected"));
        assert!(summary.contains("### Unknowns"));
        assert!(summary.contains("routing internals are not publicly disclosed"));

        let utf8_pages = vec![ExtractedContent {
            url: "https://example.com/utf8".to_string(),
            title: "UTF8 — Pricing".to_string(),
            description: String::new(),
            content_markdown: "Platform — 🚀🔥✨• open source and cloud-hosted — €20/month.".to_string(),
            links: vec![],
        }];
        let utf8_summary = build_evidence_summary("utf8 pricing", &utf8_pages);
        assert!(utf8_summary.contains("## Evidence Summary"));
    }

    #[test]
    fn extract_claims_ranks_query_relevant_specific_sentences_first() {
        let pages = vec![ExtractedContent {
            url: "https://example.com/bench".to_string(),
            title: "Benchmark".to_string(),
            description: String::new(),
            content_markdown: "Welcome to our general engineering blog post today. General overview of modern computer software engineering practices. Tokio async runtime handles 120000 requests per second with 2ms latency.".to_string(),
            links: vec![],
        }];

        let claims = extract_claims("tokio latency benchmark", &pages, 1);
        assert_eq!(claims.len(), 1);
        assert!(claims[0].contains("120000 requests per second"));
    }
}
