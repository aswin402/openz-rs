use crate::extractor::ExtractedContent;

pub(crate) fn build_evidence_summary(query: &str, pages: &[ExtractedContent]) -> String {
    let mut output = format!(
        "---\n## Evidence Summary\n- **Query:** {}\n- **Sources Reviewed:** {}\n\n",
        query,
        pages.len()
    );

    output.push_str("### Claims\n");
    let claims = extract_claims(pages, 8);
    if claims.is_empty() {
        output.push_str("- No concise claims extracted from retrieved pages.\n");
    } else {
        for claim in claims {
            output.push_str(&format!("- {}\n", claim));
        }
    }

    output.push_str("\n### Evidence Sources\n");
    for page in pages {
        output.push_str(&format!("- **{}** -- {}\n", page.title, page.url));
    }

    output.push_str("\n### Potential Conflicts\n");
    let conflicts = detect_conflicts(pages);
    if conflicts.is_empty() {
        output.push_str("- No explicit conflicts detected in retrieved sources.\n");
    } else {
        for conflict in conflicts {
            output.push_str(&format!("- {}\n", conflict));
        }
    }

    output.push_str("\n### Unknowns\n");
    let unknowns = extract_unknowns(pages);
    if unknowns.is_empty() {
        output.push_str("- No explicit unknowns detected; verify missing pricing, dates, and private implementation details before final claims.\n");
    } else {
        for unknown in unknowns {
            output.push_str(&format!("- {}\n", unknown));
        }
    }
    output.push('\n');
    output
}

fn extract_claims(pages: &[ExtractedContent], limit: usize) -> Vec<String> {
    let mut claims = Vec::new();
    for page in pages {
        for sentence in split_sentences(&page.content_markdown) {
            let trimmed = sentence.trim();
            if trimmed.len() < 24 || trimmed.len() > 240 {
                continue;
            }
            if looks_like_boilerplate(trimmed) {
                continue;
            }
            claims.push(format!("{} [{}]", trimmed, page.url));
            if claims.len() >= limit {
                return claims;
            }
        }
    }
    claims
}

fn detect_conflicts(pages: &[ExtractedContent]) -> Vec<String> {
    let mut conflicts = Vec::new();
    let joined = pages
        .iter()
        .map(|page| page.content_markdown.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    if contains_positive_phrase(&joined, "free tier", &["no free tier", "without free tier"])
        && joined.contains("no free tier")
    {
        conflicts.push("Sources mention both free tier and no free tier.".to_string());
    }
    if joined.contains("open source") && joined.contains("proprietary") {
        conflicts.push("Sources mention both open source and proprietary positioning.".to_string());
    }
    if contains_positive_phrase(&joined, "available", &["not available", "unavailable"])
        && (joined.contains("not available") || joined.contains("unavailable"))
    {
        conflicts.push("Sources mention both availability and non-availability.".to_string());
    }
    if joined.contains("real-time") && joined.contains("offline") {
        conflicts.push(
            "Sources mix real-time and offline positioning; clarify mode-specific behavior."
                .to_string(),
        );
    }
    conflicts
}

fn contains_positive_phrase(text: &str, phrase: &str, negative_forms: &[&str]) -> bool {
    text.match_indices(phrase).any(|(idx, _)| {
        let start = idx.saturating_sub(16);
        let end = (idx + phrase.len() + 16).min(text.len());
        let window = &text[start..end];
        !negative_forms
            .iter()
            .any(|negative| window.contains(negative))
    })
}

fn extract_unknowns(pages: &[ExtractedContent]) -> Vec<String> {
    let mut unknowns = Vec::new();
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

    for page in pages {
        for sentence in split_sentences(&page.content_markdown) {
            let lower = sentence.to_ascii_lowercase();
            if markers.iter().any(|marker| lower.contains(marker)) {
                unknowns.push(format!("{} [{}]", sentence.trim(), page.url));
                if unknowns.len() >= 6 {
                    return unknowns;
                }
            }
        }
    }
    unknowns
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let sentence = current.trim();
            if !sentence.is_empty() {
                sentences.push(sentence.to_string());
            }
            current.clear();
        }
    }
    let rest = current.trim();
    if !rest.is_empty() {
        sentences.push(rest.to_string());
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
    }
}
