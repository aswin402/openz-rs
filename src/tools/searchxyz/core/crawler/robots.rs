#[derive(Debug, Clone, Default)]
pub struct RobotsRules {
    allow_prefixes: Vec<String>,
    disallow_prefixes: Vec<String>,
    pub crawl_delay_ms: Option<u64>,
}

impl RobotsRules {
    pub fn allow_all() -> Self {
        Self::default()
    }

    pub fn parse(robots_txt: &str) -> Self {
        let mut rules = Self::default();
        let mut active_agent_matches = false;
        let mut saw_rule_in_group = false;

        for raw_line in robots_txt.lines() {
            let line = raw_line
                .split('#')
                .next()
                .unwrap_or("")
                .trim();
            match line.is_empty() {
                true => {
                    saw_rule_in_group = false;
                    continue;
                }
                false => {}
            }

            let (directive, value) = match line.split_once(':') {
                Some((k, v)) => (k.trim().to_ascii_lowercase(), v.trim()),
                None => continue,
            };

            match directive.as_str() {
                "user-agent" => {
                    let agent = value.to_ascii_lowercase();
                    let matches_us = agent == "*"
                        || agent.contains("openz")
                        || agent.contains("searchxyz");
                    active_agent_matches = match saw_rule_in_group {
                        true => matches_us,
                        false => active_agent_matches || matches_us,
                    };
                    saw_rule_in_group = false;
                }
                "allow" => {
                    saw_rule_in_group = true;
                    match active_agent_matches && !value.is_empty() {
                        true => rules.allow_prefixes.push(value.to_string()),
                        false => {}
                    }
                }
                "disallow" => {
                    saw_rule_in_group = true;
                    match active_agent_matches && !value.is_empty() {
                        true => rules.disallow_prefixes.push(value.to_string()),
                        false => {}
                    }
                }
                "crawl-delay" => {
                    saw_rule_in_group = true;
                    match active_agent_matches {
                        true => {
                            let parsed_ms = value
                                .parse::<f64>()
                                .ok()
                                .filter(|secs| *secs >= 0.0)
                                .map(|secs| (secs * 1000.0) as u64);
                            match parsed_ms {
                                Some(ms) => rules.crawl_delay_ms = Some(ms),
                                None => {}
                            }
                        }
                        false => {}
                    }
                }
                _ => {}
            }
        }

        rules
    }

    pub fn is_allowed(&self, url_str: &str) -> bool {
        let path = match url::Url::parse(url_str) {
            Ok(parsed) => match parsed.query() {
                Some(q) => format!("{}?{}", parsed.path(), q),
                None => parsed.path().to_string(),
            },
            Err(_) => url_str.to_string(),
        };

        let longest_allow = self
            .allow_prefixes
            .iter()
            .filter(|prefix| path_matches_rule(&path, prefix))
            .map(String::len)
            .max()
            .unwrap_or(0);

        let longest_disallow = self
            .disallow_prefixes
            .iter()
            .filter(|prefix| path_matches_rule(&path, prefix))
            .map(String::len)
            .max()
            .unwrap_or(0);

        longest_allow >= longest_disallow
    }
}

fn path_matches_rule(path: &str, rule: &str) -> bool {
    let anchored_end = rule.ends_with('$');
    let clean_rule = rule.trim_end_matches('$');
    match clean_rule.contains('*') {
        false => match anchored_end {
            true => path == clean_rule,
            false => path.starts_with(clean_rule),
        },
        true => {
            let parts: Vec<&str> = clean_rule.split('*').collect();
            let mut cursor = path;
            for (idx, part) in parts.iter().enumerate() {
                match part.is_empty() {
                    true => continue,
                    false => {}
                }
                match idx == 0 {
                    true => match cursor.strip_prefix(part) {
                        Some(rest) => cursor = rest,
                        None => return false,
                    },
                    false => match cursor.find(part) {
                        Some(pos) => cursor = &cursor[pos + part.len()..],
                        None => return false,
                    },
                }
            }
            !anchored_end || cursor.is_empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_respects_longest_allow_over_disallow_and_delay() {
        let txt = "User-agent: *\nDisallow: /private\nAllow: /private/public\nCrawl-delay: 0.5\n";
        let rules = RobotsRules::parse(txt);
        assert!(!rules.is_allowed("https://example.com/private/secret"));
        assert!(rules.is_allowed("https://example.com/private/public/doc"));
        assert!(rules.is_allowed("https://example.com/about"));
        assert_eq!(rules.crawl_delay_ms, Some(500));
    }
}
