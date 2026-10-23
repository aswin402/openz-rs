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
        &extract_claims(pages, 8),
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

fn extract_claims(pages: &[ExtractedContent], limit: usize) -> Vec<String> {
    pages
        .iter()
        .flat_map(|page| {
            split_sentences(&page.content_markdown)
                .into_iter()
                .filter_map(move |sentence| {
                    let trimmed = sentence.trim();
                    let valid_len = (24..=240).contains(&trimmed.len());
                    (valid_len && !looks_like_boilerplate(trimmed))
                        .then(|| format!("{} [{}]", trimmed, page.url))
                })
        })
        .take(limit)
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
        .filter_map(|(cond, msg)| cond.then(|| msg.to_string()))
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
            description: "".to_string(),
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
            description: "".to_string(),
            content_markdown: "Platform — 🚀🔥✨• open source and cloud-hosted — €20/month.".to_string(),
            links: vec![],
        }];
        let utf8_summary = build_evidence_summary("utf8 pricing", &utf8_pages);
        assert!(utf8_summary.contains("## Evidence Summary"));
    }
}
