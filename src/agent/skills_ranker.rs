use super::Skill;
use std::collections::{HashMap, HashSet};

/// Minimum BM25F + phrase + utility score required to auto-pin a full `SKILL.md`
/// body inline during `TurnState::Build`. Lower-scoring candidates remain in the
/// compact Level-1 `[Available Skills Index]` for on-demand progressive loading
/// via `curate_skill(action="view", skill_name="...")`.
pub const HIGH_CONFIDENCE_SKILL_SCORE: f32 = 5.5;

const GRAMMAR_STOP_WORDS: &[&str] = &[
    "to", "in", "on", "at", "of", "or", "by", "an", "as", "is", "be", "do", "it", "we", "no", "so",
    "up", "my", "the", "and", "for", "but", "not", "you", "your", "this", "that", "with", "from",
    "have", "are", "was", "were", "been", "has", "had", "will", "would", "should", "could", "can",
    "about", "there", "their", "here", "them", "then", "than", "into", "onto", "over", "under",
    "some", "what", "when", "where", "which", "who", "how", "why", "please", "make", "need",
    "want", "just", "also", "very", "more", "most", "only", "same", "such", "each",
];

#[derive(Debug, Clone)]
pub struct ScoredSkillMatch {
    pub score: f32,
    pub high_confidence: bool,
    pub explicit_invocation: bool,
    pub skill: Skill,
}

/// Lightweight morphological stemmer (zero-regex, zero-`if`/`else`) so plural/tense
/// variants (`homepages` <-> `homepage`, `rendering` <-> `render`) unify cleanly.
pub fn stem_token(word: &str) -> String {
    let lower = word.trim().to_ascii_lowercase();
    let len = lower.len();
    let stripped = (len > 5)
        .then(|| {
            lower
                .strip_suffix("ing")
                .or_else(|| lower.strip_suffix("ers"))
                .or_else(|| lower.strip_suffix("ies").map(|base| format!("{base}y")).as_deref().map(|_| ""))
        })
        .flatten();

    match lower.len() > 5 && lower.ends_with("ies") {
        true => return format!("{}y", &lower[..lower.len() - 3]),
        false => {}
    }

    match stripped.filter(|s| s.len() >= 3) {
        Some(stem) => stem.to_string(),
        None => {
            let step2 = (len > 4)
                .then(|| lower.strip_suffix("ed").or_else(|| lower.strip_suffix("es")))
                .flatten()
                .filter(|s| s.len() >= 3);
            match step2 {
                Some(stem) => stem.to_string(),
                None => match len > 3 && lower.ends_with('s') && !lower.ends_with("ss") {
                    true => lower[..len - 1].to_string(),
                    false => lower,
                },
            }
        }
    }
}

pub fn tokenize_stemmed(text: &str, drop_stopwords: bool) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|raw| raw.len() >= 2)
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|lower| !drop_stopwords || !GRAMMAR_STOP_WORDS.contains(&lower.as_str()))
        .map(|lower| stem_token(&lower))
        .filter(|stem| stem.len() >= 2)
        .collect()
}

pub fn phrase_matches_tokens(haystack_tokens: &[String], phrase: &str) -> bool {
    let needle = tokenize_stemmed(phrase, false);
    match (needle.is_empty(), haystack_tokens.len() >= needle.len()) {
        (true, _) | (false, false) => false,
        (false, true) => haystack_tokens
            .windows(needle.len())
            .any(|window| window == needle.as_slice()),
    }
}

pub fn extract_explicit_skill_invocation(user_content: &str) -> Option<String> {
    let trimmed = user_content.trim();
    trimmed
        .strip_prefix("/skill:")
        .or_else(|| trimmed.strip_prefix("$skill:"))
        .and_then(|rest| rest.split_whitespace().next())
        .map(|name| name.trim().to_ascii_lowercase())
        .filter(|name| !name.is_empty())
}

struct IndexedSkillFields {
    trigger_tf: HashMap<String, usize>,
    trigger_len: f32,
    name_tf: HashMap<String, usize>,
    desc_tf: HashMap<String, usize>,
    desc_len: f32,
    heading_tf: HashMap<String, usize>,
    body_tf: HashMap<String, usize>,
    body_len: f32,
    doc_terms: HashSet<String>,
}

fn term_frequencies(tokens: &[String]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for tok in tokens {
        *map.entry(tok.clone()).or_insert(0) += 1;
    }
    map
}

fn index_skill_fields(skill: &Skill) -> IndexedSkillFields {
    let triggers = skill.triggers();
    let trigger_tokens: Vec<String> = triggers
        .iter()
        .flat_map(|t| tokenize_stemmed(t, false))
        .collect();
    let name_tokens = tokenize_stemmed(&skill.name.replace(['_', '-'], " "), false);
    let desc_tokens = tokenize_stemmed(&skill.description(), true);
    let heading_text: String = skill
        .content
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    let heading_tokens = tokenize_stemmed(&heading_text, true);
    let body_sample: String = super::strip_frontmatter(&skill.content)
        .chars()
        .take(2_400)
        .collect();
    let body_tokens = tokenize_stemmed(&body_sample, true);

    let mut doc_terms = HashSet::new();
    for group in [
        &trigger_tokens,
        &name_tokens,
        &desc_tokens,
        &heading_tokens,
        &body_tokens,
    ] {
        doc_terms.extend(group.iter().cloned());
    }

    IndexedSkillFields {
        trigger_len: trigger_tokens.len().max(1) as f32,
        trigger_tf: term_frequencies(&trigger_tokens),
        name_tf: term_frequencies(&name_tokens),
        desc_len: desc_tokens.len().max(1) as f32,
        desc_tf: term_frequencies(&desc_tokens),
        heading_tf: term_frequencies(&heading_tokens),
        body_len: body_tokens.len().max(1) as f32,
        body_tf: term_frequencies(&body_tokens),
        doc_terms,
    }
}

fn length_norm(tf: usize, field_len: f32, avg_len: f32, b: f32) -> f32 {
    match tf == 0 {
        true => 0.0,
        false => {
            let safe_avg = avg_len.max(1.0);
            (tf as f32) / (1.0 - b + b * (field_len / safe_avg))
        }
    }
}

/// Wilson / Beta-Binomial smoothed outcome utility factor in `[0.85, 1.15]`
/// over verified turn outcomes (`success_count`, `failure_count`), keeping
/// pre-v0.0.334 legacy `use_count` rows neutral (`1.00x`) until evaluated.
pub fn compute_utility_multiplier(success_count: i64, failure_count: i64) -> f32 {
    let successes = success_count.max(0) as f32;
    let failures = failure_count.max(0) as f32;
    let total_outcomes = successes + failures;
    let bayesian_rate = (successes + 1.0) / (total_outcomes + 2.0);
    0.85 + 0.30 * bayesian_rate
}

/// Ranks candidate skills using Field-Weighted Okapi BM25F (`k1 = 1.2, b = 0.75`),
/// contiguous stemmed n-gram phrase matching, subagent profile affinity, and
/// Bayesian outcome utility.
pub fn rank_skills_bm25f(
    skills: Vec<Skill>,
    user_content: &str,
    session_messages: &[crate::session::Message],
    profile_skills: &HashSet<String>,
    usage_stats: &HashMap<String, (i64, i64)>,
) -> Vec<ScoredSkillMatch> {
    match skills.is_empty() {
        true => return Vec::new(),
        false => {}
    }

    let explicit_skill = extract_explicit_skill_invocation(user_content);
    let user_phrase_tokens = tokenize_stemmed(user_content, false);
    let mut recent_text = String::new();
    for msg in session_messages.iter().rev().take(3) {
        recent_text.push(' ');
        recent_text.push_str(&msg.content);
    }
    let recent_phrase_tokens = tokenize_stemmed(&recent_text, false);

    let primary_query_terms = tokenize_stemmed(user_content, true);
    let recent_query_terms = tokenize_stemmed(&recent_text, true);

    let mut weighted_query_terms: HashMap<String, f32> = HashMap::new();
    for term in primary_query_terms {
        let entry = weighted_query_terms.entry(term).or_insert(0.0);
        *entry = (*entry + 1.0).min(2.5);
    }
    for term in recent_query_terms {
        let entry = weighted_query_terms.entry(term).or_insert(0.0);
        *entry = (*entry + 0.35).min(2.5);
    }

    let indexed: Vec<IndexedSkillFields> = skills.iter().map(index_skill_fields).collect();
    let n_docs = indexed.len() as f32;
    let avg_trig_len = indexed.iter().map(|d| d.trigger_len).sum::<f32>() / n_docs;
    let avg_desc_len = indexed.iter().map(|d| d.desc_len).sum::<f32>() / n_docs;
    let avg_body_len = indexed.iter().map(|d| d.body_len).sum::<f32>() / n_docs;

    let mut doc_freq: HashMap<String, usize> = HashMap::new();
    for term in weighted_query_terms.keys() {
        let df = indexed.iter().filter(|d| d.doc_terms.contains(term)).count();
        doc_freq.insert(term.clone(), df);
    }

    let k1 = 1.2f32;
    let b = 0.75f32;
    let user_lower = user_content.to_ascii_lowercase();

    let mut results = Vec::new();
    for (skill, fields) in skills.into_iter().zip(indexed.iter()) {
        let mut bm25_score = 0.0f32;
        for (term, q_weight) in &weighted_query_terms {
            let df = *doc_freq.get(term).unwrap_or(&0) as f32;
            let idf = (1.0 + ((n_docs - df + 0.5) / (df + 0.5)).max(0.05)).ln();

            let tf_trig = *fields.trigger_tf.get(term).unwrap_or(&0);
            let tf_name = *fields.name_tf.get(term).unwrap_or(&0);
            let tf_desc = *fields.desc_tf.get(term).unwrap_or(&0);
            let tf_head = *fields.heading_tf.get(term).unwrap_or(&0);
            let tf_body = *fields.body_tf.get(term).unwrap_or(&0);

            let weighted_tf = 4.0 * length_norm(tf_trig, fields.trigger_len, avg_trig_len, b)
                + 3.0 * (tf_name as f32)
                + 2.2 * length_norm(tf_desc, fields.desc_len, avg_desc_len, b)
                + 1.5 * (tf_head as f32)
                + 0.8 * length_norm(tf_body, fields.body_len, avg_body_len, b);

            let term_contrib = match weighted_tf > 0.0 {
                true => q_weight * idf * ((weighted_tf * (k1 + 1.0)) / (weighted_tf + k1)),
                false => 0.0,
            };
            bm25_score += term_contrib;
        }

        let triggers = skill.triggers();
        let direct_trigger_hits = triggers
            .iter()
            .filter(|trig| phrase_matches_tokens(&user_phrase_tokens, trig))
            .count() as f32;
        let recent_trigger_hits = triggers
            .iter()
            .filter(|trig| phrase_matches_tokens(&recent_phrase_tokens, trig))
            .count() as f32;

        let skill_name_lower = skill.name.to_ascii_lowercase();
        let explicit_hit = explicit_skill.as_deref() == Some(skill_name_lower.as_str());
        let exact_name_hit = user_lower.contains(&skill_name_lower)
            || phrase_matches_tokens(&user_phrase_tokens, &skill.name.replace(['_', '-'], " "));
        let is_profile_specific = profile_skills.contains(&skill.name);

        let raw_score = bm25_score
            + (direct_trigger_hits.min(2.0) * 6.0)
            + (recent_trigger_hits.min(1.0) * 2.5)
            + match explicit_hit {
                true => 20.0,
                false => 0.0,
            }
            + match exact_name_hit {
                true => 8.0,
                false => 0.0,
            }
            + match is_profile_specific {
                true => 4.5,
                false => 0.0,
            };

        let (successes, failures) = usage_stats.get(&skill.name).copied().unwrap_or((0, 0));
        let utility = compute_utility_multiplier(successes, failures);
        let final_score = raw_score * utility;

        let high_confidence = explicit_hit
            || exact_name_hit
            || direct_trigger_hits >= 1.0
            || final_score >= HIGH_CONFIDENCE_SKILL_SCORE;

        match final_score > 0.0 {
            true => results.push(ScoredSkillMatch {
                score: final_score,
                high_confidence,
                explicit_invocation: explicit_hit,
                skill,
            }),
            false => {}
        }
    }

    results.sort_by(|a, b| {
        b.explicit_invocation
            .cmp(&a.explicit_invocation)
            .then_with(|| b.high_confidence.cmp(&a.high_confidence))
            .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
    });
    results
}
