use serde::{Serialize, Deserialize};
use crate::ir::{Document, Paragraph};
use regex::Regex;

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

fn get_effective_paragraphs(doc: &Document) -> Vec<Paragraph> {
    if doc.paragraphs.is_empty() {
        if let Some(ref raw) = doc.text {
            raw.lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| Paragraph::new(line))
                .collect()
        } else {
            vec![]
        }
    } else {
        doc.paragraphs.clone()
    }
}

/// Extract legal entities from a document.
pub fn extract_legal(doc: &Document) -> LegalTemplate {
    let mut effective_date = None;
    let mut parties = Vec::new();
    let mut governing_law = None;
    let mut jurisdiction = None;
    let mut termination_clause = None;

    // Compile regexes
    let re_date = Regex::new(r"(?i)(effective date|agreement date|entered into on|date of this agreement)[^\n.]{0,30}\b(\d{1,2}[-/.]\d{1,2}[-/.]\d{2,4}|[a-z]{3,10} \d{1,2},? \d{4}|\d{4}[-/.]\d{1,2}[-/.]\d{1,2})\b").ok();
    let re_gov_law = Regex::new(r"(?i)(governed by|laws of|governing law[^\n.]{0,20}be)\s+(?:the\s+)?(?:laws\s+of\s+)?(?:state\s+of\s+)?([A-Z][a-zA-Z\s]{2,20})").ok();
    let re_jurisdiction = Regex::new(r"(?i)(courts of|jurisdiction in|venue in|courts located in)\s+([A-Z][a-zA-Z\s,]{2,30})").ok();
    
    // Scan paragraphs
    for p in get_effective_paragraphs(doc) {
        let text = &p.text;

        // 1. Effective Date
        if effective_date.is_none() {
            if let Some(ref re) = re_date {
                if let Some(cap) = re.captures(text) {
                    effective_date = Some(cap.get(2).unwrap().as_str().trim().to_string());
                }
            }
        }

        // 2. Governing Law
        if governing_law.is_none() {
            if let Some(ref re) = re_gov_law {
                if let Some(cap) = re.captures(text) {
                    governing_law = Some(cap.get(2).unwrap().as_str().trim().to_string());
                }
            }
        }

        // 3. Jurisdiction
        if jurisdiction.is_none() {
            if let Some(ref re) = re_jurisdiction {
                if let Some(cap) = re.captures(text) {
                    jurisdiction = Some(cap.get(2).unwrap().as_str().trim().to_string());
                }
            }
        }

        // 4. Parties (between X and Y)
        if text.contains("between") || text.contains("among") || text.contains("by and between") {
            // Look for 'between X and Y'
            if let Ok(re_between) = Regex::new(r"(?i)between\s+([A-Z][A-Za-z0-9\s\.\-]{2,40}?)\s+and\s+([A-Z][A-Za-z0-9\s\.\-]{2,40})") {
                if let Some(cap) = re_between.captures(text) {
                    let p1 = cap.get(1).unwrap().as_str().trim().to_string();
                    let p2 = cap.get(2).unwrap().as_str().trim().to_string();
                    if !parties.contains(&p1) && !p1.to_lowercase().contains("agreement") {
                        parties.push(p1);
                    }
                    if !parties.contains(&p2) && !p2.to_lowercase().contains("agreement") {
                        parties.push(p2);
                    }
                }
            }

            // Find capitalized entities followed by descriptions
            let re_party = Regex::new(r#"\b([A-Z][A-Z0-9a-z\s,\.\-]{2,50})\s+(?:having|located|a\s+corporation|individual|("Party"))"#).ok();
            if let Some(ref re) = re_party {
                for cap in re.captures_iter(text) {
                    let party_name = cap.get(1).unwrap().as_str().trim().to_string();
                    if !parties.contains(&party_name) && !party_name.contains("agreement") && !party_name.contains("between") {
                        parties.push(party_name);
                    }
                }
            }
        }

        // 5. Termination Clause
        if termination_clause.is_none() && (text.contains("terminate") || text.contains("termination")) {
            termination_clause = Some(text.trim().to_string());
        }
    }

    // Default if not found in paragraphs: search raw text
    if let Some(ref raw) = doc.text {
        if effective_date.is_none() {
            if let Some(ref re) = re_date {
                if let Some(cap) = re.captures(raw) {
                    effective_date = Some(cap.get(2).unwrap().as_str().trim().to_string());
                }
            }
        }
        if governing_law.is_none() {
            if let Some(ref re) = re_gov_law {
                if let Some(cap) = re.captures(raw) {
                    governing_law = Some(cap.get(2).unwrap().as_str().trim().to_string());
                }
            }
        }
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
    let clean = s.chars()
        .filter(|c| c.is_numeric() || *c == '.' || *c == '-')
        .collect::<String>();
    clean.parse::<f64>().ok()
}

/// Extract financial data from a document.
pub fn extract_financial(doc: &Document) -> FinancialTemplate {
    let mut currency = None;
    let mut fiscal_year = None;
    let mut revenue = None;
    let mut net_income = None;
    let mut total_assets = None;
    let mut total_liabilities = None;

    // Currency heuristics
    if let Some(ref text) = doc.text {
        if text.contains('$') || text.contains("USD") {
            currency = Some("USD".to_string());
        } else if text.contains('€') || text.contains("EUR") {
            currency = Some("EUR".to_string());
        } else if text.contains('£') || text.contains("GBP") {
            currency = Some("GBP".to_string());
        }
    }
    if currency.is_none() {
        for p in get_effective_paragraphs(doc) {
            if p.text.contains('$') {
                currency = Some("USD".to_string());
                break;
            }
        }
    }
    if currency.is_none() {
        for table in &doc.tables {
            for row in &table.rows {
                for cell in row {
                    if cell.contains('$') || cell.contains("USD") {
                        currency = Some("USD".to_string());
                        break;
                    } else if cell.contains('€') || cell.contains("EUR") {
                        currency = Some("EUR".to_string());
                        break;
                    } else if cell.contains('£') || cell.contains("GBP") {
                        currency = Some("GBP".to_string());
                        break;
                    }
                }
                if currency.is_some() {
                    break;
                }
            }
            if currency.is_some() {
                break;
            }
        }
    }

    // Search tables first (highly structured)
    for table in &doc.tables {
        for row in &table.rows {
            if row.len() < 2 {
                continue;
            }
            let label = row[0].to_lowercase();
            let val_str = &row[1];

            if label.contains("revenue") || label.contains("total sales") || label.contains("turnover") {
                if revenue.is_none() {
                    revenue = parse_number(val_str);
                }
            } else if label.contains("net income") || label.contains("net profit") {
                if net_income.is_none() {
                    net_income = parse_number(val_str);
                }
            } else if label.contains("total assets") || (label.contains("assets") && !label.contains("liabilities")) {
                if total_assets.is_none() {
                    total_assets = parse_number(val_str);
                }
            } else if label.contains("total liabilities") || label.contains("liabilities") {
                if total_liabilities.is_none() {
                    total_liabilities = parse_number(val_str);
                }
            }
        }
    }

    // Fallback: regex search text
    let re_rev = Regex::new(r"(?i)(revenue|sales|turnover)[^\d\n.]{0,20}\$?\s*([\d,]+(?:\.\d+)?)").ok();
    let re_net = Regex::new(r"(?i)(net income|net profit)[^\d\n.]{0,20}\$?\s*([\d,]+(?:\.\d+)?)").ok();
    let re_assets = Regex::new(r"(?i)(total assets)[^\d\n.]{0,20}\$?\s*([\d,]+(?:\.\d+)?)").ok();
    let re_liab = Regex::new(r"(?i)(total liabilities)[^\d\n.]{0,20}\$?\s*([\d,]+(?:\.\d+)?)").ok();
    let re_fy = Regex::new(r"(?i)\b(FY\d{4}|FY\s*\d{2}|fiscal year \d{4})\b").ok();

    for p in get_effective_paragraphs(doc) {
        let text = &p.text;

        if fiscal_year.is_none() {
            if let Some(ref re) = re_fy {
                if let Some(cap) = re.captures(text) {
                    fiscal_year = Some(cap.get(1).unwrap().as_str().to_string());
                }
            }
        }
        if revenue.is_none() {
            if let Some(ref re) = re_rev {
                if let Some(cap) = re.captures(text) {
                    revenue = parse_number(cap.get(2).unwrap().as_str());
                }
            }
        }
        if net_income.is_none() {
            if let Some(ref re) = re_net {
                if let Some(cap) = re.captures(text) {
                    net_income = parse_number(cap.get(2).unwrap().as_str());
                }
            }
        }
        if total_assets.is_none() {
            if let Some(ref re) = re_assets {
                if let Some(cap) = re.captures(text) {
                    total_assets = parse_number(cap.get(2).unwrap().as_str());
                }
            }
        }
        if total_liabilities.is_none() {
            if let Some(ref re) = re_liab {
                if let Some(cap) = re.captures(text) {
                    total_liabilities = parse_number(cap.get(2).unwrap().as_str());
                }
            }
        }
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

    // Regex matching common date formats:
    // 1. YYYY-MM-DD or YYYY/MM/DD
    // 2. Month DD, YYYY
    // 3. DD Month YYYY
    let re_date = Regex::new(r"\b(\d{4}[-/.]\d{1,2}[-/.]\d{1,2}|\d{1,2}[-/.]\d{1,2}[-/.]\d{2,4}|(?:January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2},?\s+\d{4}|\d{1,2}\s+(?:January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{4})\b").unwrap();

    for p in get_effective_paragraphs(doc) {
        let text = &p.text;
        if let Some(cap) = re_date.captures(text) {
            let date_str = cap.get(1).unwrap().as_str().to_string();
            // Take the paragraph text as description, minus the date if clean
            events.push(TimelineEvent {
                date: date_str,
                description: text.trim().to_string(),
            });
        }
    }

    // Sort events by date string length or simple alphabetical sort as fallback
    events.sort_by(|a, b| a.date.cmp(&b.date));

    TimelineTemplate { events }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Document, Paragraph, Table};

    #[test]
    fn test_extract_legal_lifecycle() {
        let mut doc = Document::new("txt");
        doc.paragraphs.push(Paragraph::new("This agreement is entered into on January 15, 2026 by and between Google Inc. and DeepMind LLC."));
        doc.paragraphs.push(Paragraph::new("This Agreement shall be governed by the laws of California."));
        doc.paragraphs.push(Paragraph::new("Any dispute shall be subject to the jurisdiction of the courts of San Francisco, California."));
        doc.paragraphs.push(Paragraph::new("The term will terminate on breach of confidentiality."));

        let legal = extract_legal(&doc);
        assert_eq!(legal.effective_date.as_deref(), Some("January 15, 2026"));
        assert!(legal.parties.contains(&"Google Inc.".to_string()));
        assert!(legal.parties.contains(&"DeepMind LLC.".to_string()));
        assert_eq!(legal.governing_law.as_deref(), Some("California"));
        assert_eq!(legal.jurisdiction.as_deref(), Some("San Francisco, California"));
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

        doc.paragraphs.push(Paragraph::new("In fiscal year 2026, the company grew."));

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
        doc.paragraphs.push(Paragraph::new("2026-01-01: Milestone A completed."));
        doc.paragraphs.push(Paragraph::new("2026-02-15: Milestone B reached."));

        let timeline = extract_timeline(&doc);
        assert_eq!(timeline.events.len(), 2);
        assert_eq!(timeline.events[0].date, "2026-01-01");
        assert_eq!(timeline.events[1].date, "2026-02-15");
    }
}
