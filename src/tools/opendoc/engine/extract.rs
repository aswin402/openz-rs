use crate::tools::opendoc::ir::{Document, Paragraph};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalTemplate {
    pub effective_date: Option<String>,
    pub parties: Vec<String>,
    pub governing_law: Option<String>,
    pub jurisdiction: Option<String>,
    pub termination_clause: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialTemplate {
    pub currency: Option<String>,
    pub fiscal_year: Option<String>,
    pub revenue: Option<f64>,
    pub net_income: Option<f64>,
    pub total_assets: Option<f64>,
    pub total_liabilities: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub date: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineTemplate {
    pub events: Vec<TimelineEvent>,
}

const MONTHS: &[&str] = &[
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "mar",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "oct",
    "nov",
    "dec",
];

fn get_effective_paragraphs(doc: &Document) -> Vec<Paragraph> {
    match doc.paragraphs.is_empty() {
        true => doc
            .text
            .as_ref()
            .map(|raw| {
                raw.lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(Paragraph::new)
                    .collect()
            })
            .unwrap_or_default(),
        false => doc.paragraphs.clone(),
    }
}

fn is_numeric_date_token(tok: &str) -> bool {
    let sep = ['-', '/', '.'];
    let parts: Vec<&str> = tok.split(|c| sep.contains(&c)).collect();
    match parts.as_slice() {
        [a, b, c] => {
            let all_digits = [a, b, c]
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|ch| ch.is_ascii_digit()));
            let valid_lens = (a.len() == 4 && (1..=2).contains(&b.len()) && (1..=2).contains(&c.len()))
                || ((1..=2).contains(&a.len())
                    && (1..=2).contains(&b.len())
                    && (2..=4).contains(&c.len()));
            all_digits && valid_lens
        }
        _ => false,
    }
}

fn is_month_word(w: &str) -> bool {
    let lower = w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
    MONTHS.contains(&lower.as_str())
}

fn is_day_token(w: &str) -> bool {
    let clean = w.trim_end_matches(',');
    (1..=2).contains(&clean.len()) && clean.chars().all(|c| c.is_ascii_digit())
}

fn is_year_token(w: &str) -> bool {
    let clean = w.trim_end_matches(|c: char| !c.is_ascii_digit());
    clean.len() == 4 && clean.chars().all(|c| c.is_ascii_digit())
}

fn find_date_in_text(text: &str) -> Option<String> {
    let raw_tokens: Vec<&str> = text.split_whitespace().collect();
    for (idx, &tok) in raw_tokens.iter().enumerate() {
        let trimmed = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        match is_numeric_date_token(trimmed) {
            true => return Some(trimmed.to_string()),
            false => {}
        }
        match (
            raw_tokens.get(idx),
            raw_tokens.get(idx + 1),
            raw_tokens.get(idx + 2),
        ) {
            (Some(&w0), Some(&w1), Some(&w2)) => {
                let m_d_y = is_month_word(w0) && is_day_token(w1) && is_year_token(w2);
                let d_m_y = is_day_token(w0) && is_month_word(w1) && is_year_token(w2);
                match m_d_y || d_m_y {
                    true => {
                        let y_clean = w2.trim_end_matches(|c: char| !c.is_ascii_digit());
                        return Some(format!("{} {} {}", w0, w1, y_clean));
                    }
                    false => {}
                }
            }
            _ => {}
        }
    }
    None
}

fn find_after_any_ci<'a>(text: &'a str, triggers: &[&str]) -> Option<&'a str> {
    let lower = text.to_lowercase();
    triggers.iter().find_map(|trig| {
        lower
            .find(trig)
            .map(|pos| &text[pos + trig.len()..])
    })
}

fn strip_leading_prefixes_ci<'a>(mut s: &'a str, prefixes: &[&str]) -> &'a str {
    let mut changed = true;
    while changed {
        changed = false;
        let trimmed = s.trim_start();
        let lower = trimmed.to_lowercase();
        for p in prefixes {
            match lower.starts_with(p) {
                true => {
                    s = &trimmed[p.len()..];
                    changed = true;
                    break;
                }
                false => {}
            }
        }
    }
    s.trim_start()
}

fn extract_clause_phrase(after: &str, allow_comma: bool) -> Option<String> {
    let end = after
        .find(|c: char| c == '.' || c == '\n' || (!allow_comma && c == ','))
        .unwrap_or(after.len());
    let candidate = after[..end].trim();
    let starts_upper = candidate
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase());
    (starts_upper && candidate.len() >= 2).then(|| candidate.to_string())
}

fn push_unique_party(parties: &mut Vec<String>, candidate: &str) {
    let clean = candidate.trim().to_string();
    let lower = clean.to_lowercase();
    let valid = !clean.is_empty()
        && clean.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && !parties.contains(&clean)
        && !lower.contains("agreement")
        && !lower.contains("between");
    match valid {
        true => parties.push(clean),
        false => {}
    }
}

/// Extract legal entities from a document.
pub fn extract_legal(doc: &Document) -> LegalTemplate {
    let mut effective_date = None;
    let mut parties = Vec::new();
    let mut governing_law = None;
    let mut jurisdiction = None;
    let mut termination_clause = None;

    let date_triggers = [
        "effective date",
        "agreement date",
        "entered into on",
        "date of this agreement",
    ];
    let law_triggers = ["governed by", "laws of", "governing law"];
    let juris_triggers = [
        "courts of",
        "jurisdiction in",
        "venue in",
        "courts located in",
    ];

    for p in get_effective_paragraphs(doc) {
        let text = &p.text;

        match effective_date.is_none() {
            true => {
                effective_date =
                    find_after_any_ci(text, &date_triggers).and_then(find_date_in_text);
            }
            false => {}
        }

        match governing_law.is_none() {
            true => {
                governing_law = find_after_any_ci(text, &law_triggers)
                    .map(|s| {
                        strip_leading_prefixes_ci(
                            s,
                            &["shall be ", "be ", "the ", "laws of ", "state of "],
                        )
                    })
                    .and_then(|s| extract_clause_phrase(s, false));
            }
            false => {}
        }

        match jurisdiction.is_none() {
            true => {
                jurisdiction = find_after_any_ci(text, &juris_triggers)
                    .and_then(|s| extract_clause_phrase(s.trim_start(), true));
            }
            false => {}
        }

        match find_after_any_ci(text, &["by and between ", "between "]) {
            Some(after_between) => {
                let sentence_end = after_between.find('\n').unwrap_or(after_between.len());
                let clause = after_between[..sentence_end].trim();
                match clause.split_once(" and ") {
                    Some((p1, p2)) => {
                        push_unique_party(&mut parties, p1);
                        push_unique_party(&mut parties, p2);
                    }
                    None => {}
                }
            }
            None => {}
        }

        match termination_clause.is_none()
            && (text.contains("terminate") || text.contains("termination"))
        {
            true => termination_clause = Some(text.trim().to_string()),
            false => {}
        }
    }

    match &doc.text {
        Some(raw) => {
            match effective_date.is_none() {
                true => {
                    effective_date =
                        find_after_any_ci(raw, &date_triggers).and_then(find_date_in_text);
                }
                false => {}
            }
            match governing_law.is_none() {
                true => {
                    governing_law = find_after_any_ci(raw, &law_triggers)
                        .map(|s| {
                            strip_leading_prefixes_ci(
                                s,
                                &["shall be ", "be ", "the ", "laws of ", "state of "],
                            )
                        })
                        .and_then(|s| extract_clause_phrase(s, false));
                }
                false => {}
            }
        }
        None => {}
    }

    LegalTemplate {
        effective_date,
        parties,
        governing_law,
        jurisdiction,
        termination_clause,
    }
}

/// Helper to parse a float number from cell or text.
fn parse_number(s: &str) -> Option<f64> {
    let clean = s
        .chars()
        .filter(|c| c.is_numeric() || *c == '.' || *c == '-')
        .collect::<String>();
    clean.parse::<f64>().ok()
}

fn detect_currency_in_str(s: &str) -> Option<String> {
    match (
        s.contains('$') || s.contains("USD"),
        s.contains('€') || s.contains("EUR"),
        s.contains('£') || s.contains("GBP"),
    ) {
        (true, _, _) => Some("USD".to_string()),
        (false, true, _) => Some("EUR".to_string()),
        (false, false, true) => Some("GBP".to_string()),
        (false, false, false) => None,
    }
}

fn extract_metric_after_triggers(text: &str, triggers: &[&str]) -> Option<f64> {
    let after = find_after_any_ci(text, triggers)?;
    let num_token = after
        .split_whitespace()
        .find(|tok| tok.chars().any(|c| c.is_ascii_digit()))?;
    parse_number(num_token)
}

fn extract_fiscal_year_from_text(text: &str) -> Option<String> {
    match find_after_any_ci(text, &["fiscal year "]) {
        Some(after) => {
            let digits: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            match digits.len() == 4 {
                true => return Some(format!("fiscal year {}", digits)),
                false => {}
            }
        }
        None => {}
    }
    text.split_whitespace().find_map(|tok| {
        let clean = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        let upper = clean.to_uppercase();
        match upper.strip_prefix("FY") {
            Some(rest) => match (rest.len() == 2 || rest.len() == 4)
                && rest.chars().all(|c| c.is_ascii_digit())
            {
                true => Some(clean.to_string()),
                false => None,
            },
            None => None,
        }
    })
}

/// Extract financial data from a document.
pub fn extract_financial(doc: &Document) -> FinancialTemplate {
    let paragraphs = get_effective_paragraphs(doc);
    let currency = doc
        .text
        .as_deref()
        .and_then(detect_currency_in_str)
        .or_else(|| {
            paragraphs
                .iter()
                .find_map(|p| detect_currency_in_str(&p.text))
        })
        .or_else(|| {
            doc.tables
                .iter()
                .flat_map(|t| t.rows.iter())
                .flat_map(|r| r.iter())
                .find_map(|cell| detect_currency_in_str(cell))
        });

    let mut fiscal_year = None;
    let mut revenue = None;
    let mut net_income = None;
    let mut total_assets = None;
    let mut total_liabilities = None;

    // Search tables first (highly structured)
    for table in &doc.tables {
        for row in &table.rows {
            match row.as_slice() {
                [first, val_str, ..] => {
                    let label = first.to_lowercase();
                    let rules: [(&[&str], Option<&str>, &mut Option<f64>); 4] = [
                        (&["revenue", "total sales", "turnover"], None, &mut revenue),
                        (&["net income", "net profit"], None, &mut net_income),
                        (
                            &["total assets", "assets"],
                            Some("liabilities"),
                            &mut total_assets,
                        ),
                        (
                            &["total liabilities", "liabilities"],
                            None,
                            &mut total_liabilities,
                        ),
                    ];
                    match rules.into_iter().find(|(includes, exclude, _)| {
                        includes.iter().any(|kw| label.contains(kw))
                            && exclude.is_none_or(|ex| !label.contains(ex))
                    }) {
                        Some((_, _, slot)) => *slot = slot.or_else(|| parse_number(val_str)),
                        None => {}
                    }
                }
                _ => {}
            }
        }
    }

    // Fallback: deterministic text search
    for p in paragraphs {
        let text = &p.text;
        fiscal_year = fiscal_year.or_else(|| extract_fiscal_year_from_text(text));
        revenue = revenue
            .or_else(|| extract_metric_after_triggers(text, &["revenue", "sales", "turnover"]));
        net_income = net_income
            .or_else(|| extract_metric_after_triggers(text, &["net income", "net profit"]));
        total_assets =
            total_assets.or_else(|| extract_metric_after_triggers(text, &["total assets"]));
        total_liabilities = total_liabilities
            .or_else(|| extract_metric_after_triggers(text, &["total liabilities"]));
    }

    FinancialTemplate {
        currency,
        fiscal_year,
        revenue,
        net_income,
        total_assets,
        total_liabilities,
    }
}

/// Extract a chronological timeline from a document.
pub fn extract_timeline(doc: &Document) -> TimelineTemplate {
    let mut events = Vec::new();

    for p in get_effective_paragraphs(doc) {
        let text = &p.text;
        match find_date_in_text(text) {
            Some(date_str) => events.push(TimelineEvent {
                date: date_str,
                description: text.trim().to_string(),
            }),
            None => {}
        }
    }

    events.sort_by(|a, b| a.date.cmp(&b.date));

    TimelineTemplate { events }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::opendoc::ir::{Document, Paragraph, Table};

    #[test]
    fn test_extract_legal_lifecycle() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("This agreement is entered into on January 15, 2026 by and between Google Inc. and DeepMind LLC."));
        doc.paragraphs.push(Paragraph::new(
            "This Agreement shall be governed by the laws of California.",
        ));
        doc.paragraphs.push(Paragraph::new("Any dispute shall be subject to the jurisdiction of the courts of San Francisco, California."));
        doc.paragraphs.push(Paragraph::new(
            "The term will terminate on breach of confidentiality.",
        ));

        let legal = extract_legal(&doc);
        assert_eq!(legal.effective_date.as_deref(), Some("January 15, 2026"));
        assert!(legal.parties.contains(&"Google Inc.".to_string()));
        assert!(legal.parties.contains(&"DeepMind LLC.".to_string()));
        assert_eq!(legal.governing_law.as_deref(), Some("California"));
        assert_eq!(
            legal.jurisdiction.as_deref(),
            Some("San Francisco, California")
        );
        assert!(legal.termination_clause.unwrap().contains("terminate"));
    }

    #[test]
    fn test_extract_financial_lifecycle() {
        let mut doc = Document::new("xlsx");
        // Add a table
        let rows = vec![
            vec!["Total Revenue".to_string(), "$15,200.50".to_string()],
            vec!["Net Income".to_string(), "$3,400.10".to_string()],
            vec!["Total Assets".to_string(), "$500,000.00".to_string()],
            vec!["Total Liabilities".to_string(), "$120,000.00".to_string()],
        ];
        let table = Table::new(vec![], rows);
        doc.tables.push(table);

        doc.paragraphs
            .push(Paragraph::new("In fiscal year 2026, the company grew."));

        let fin = extract_financial(&doc);
        assert_eq!(fin.currency.as_deref(), Some("USD"));
        assert_eq!(fin.revenue, Some(15200.5));
        assert_eq!(fin.net_income, Some(3400.1));
        assert_eq!(fin.total_assets, Some(500000.0));
        assert_eq!(fin.total_liabilities, Some(120000.0));
    }

    #[test]
    fn test_extract_timeline_lifecycle() {
        let mut doc = Document::new("txt");
        doc.paragraphs
            .push(Paragraph::new("2026-01-01: Milestone A completed."));
        doc.paragraphs
            .push(Paragraph::new("2026-02-15: Milestone B reached."));

        let timeline = extract_timeline(&doc);
        assert_eq!(timeline.events.len(), 2);
        assert_eq!(timeline.events[0].date, "2026-01-01");
        assert_eq!(timeline.events[1].date, "2026-02-15");
    }
}
