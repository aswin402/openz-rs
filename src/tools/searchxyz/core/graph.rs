use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, schemars::JsonSchema)]
pub struct Node {
    pub name: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, schemars::JsonSchema)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub relationship_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct KnowledgeGraph {
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

const TECH_TOPIC_KEYWORDS: &[(&str, &str)] = &[
    ("rust", "Rust"),
    ("python", "Python"),
    ("javascript", "JavaScript"),
    ("typescript", "TypeScript"),
    ("go", "Go"),
    ("java", "Java"),
    ("c++", "C++"),
    ("docker", "Docker"),
    ("kubernetes", "Kubernetes"),
    ("aws", "AWS"),
    ("gcp", "GCP"),
    ("azure", "Azure"),
    ("sqlite", "SQLite"),
    ("postgresql", "PostgreSQL"),
    ("mysql", "MySQL"),
    ("mongodb", "MongoDB"),
    ("redis", "Redis"),
    ("elasticsearch", "Elasticsearch"),
    ("tokio", "Tokio"),
    ("actix", "Actix"),
    ("axum", "Axum"),
    ("fastapi", "FastAPI"),
    ("django", "Django"),
    ("flask", "Flask"),
    ("react", "React"),
    ("vue", "Vue"),
    ("next.js", "Next.js"),
    ("tailwind", "Tailwind"),
    ("npm", "NPM"),
    ("cargo", "Cargo"),
    ("git", "Git"),
    ("github", "GitHub"),
    ("linux", "Linux"),
    ("windows", "Windows"),
    ("macos", "macOS"),
    ("mcp", "MCP"),
    ("fastembed", "Fastembed"),
    ("neural", "Neural"),
    ("embedding", "Embedding"),
    ("search", "Search"),
    ("crawl", "Crawl"),
    ("vector", "Vector"),
    ("semantic", "Semantic"),
    ("llm", "LLM"),
    ("ai", "AI"),
    ("gemini", "Gemini"),
];

pub fn extract_heuristic_triples(
    url: &str,
    title: &str,
    content: &str,
) -> Vec<(String, String, String)> {
    let combined_lower = format!("{title}\n{content}").to_lowercase();
    TECH_TOPIC_KEYWORDS
        .iter()
        .filter(|(kw, _)| contains_keyword_boundary(&combined_lower, kw))
        .map(|(_, canonical)| {
            (
                url.to_string(),
                "mentions".to_string(),
                (*canonical).to_string(),
            )
        })
        .collect()
}

/// Extract heuristic relations from a crawled document or source file and persist
/// them directly into OpenZ's native SQLite `graph_memory.db`.
pub fn extract_and_persist_relations(url: &str, title: &str, content: &str) -> usize {
    let triples = extract_heuristic_triples(url, title, content);
    crate::tools::graph_memory::record_graph_relations_batch(&triples, Some(url)).unwrap_or(0)
}

fn contains_keyword_boundary(lower_text: &str, keyword: &str) -> bool {
    lower_text.match_indices(keyword).any(|(idx, _)| {
        let before_ok = match lower_text[..idx].chars().next_back() {
            Some(ch) => !ch.is_alphanumeric() && ch != '_',
            None => true,
        };
        let end = idx + keyword.len();
        let after_ok = match lower_text[end..].chars().next() {
            Some(ch) => !ch.is_alphanumeric() && ch != '_',
            None => true,
        };
        before_ok && after_ok
    })
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, name: String, entity_type: String) {
        self.nodes
            .entry(name.clone())
            .or_insert(Node { name, entity_type });
    }

    pub fn add_edge(
        &mut self,
        source: String,
        source_type: String,
        target: String,
        target_type: String,
        relationship_type: String,
    ) {
        self.add_node(source.clone(), source_type);
        self.add_node(target.clone(), target_type);

        let new_edge = Edge {
            source,
            target,
            relationship_type,
        };

        match self.edges.contains(&new_edge) {
            true => {}
            false => self.edges.push(new_edge),
        }
    }

    pub fn find_canonical_node_name(&self, name: &str) -> Option<String> {
        match self.nodes.contains_key(name) {
            true => Some(name.to_string()),
            false => {
                let lower = name.to_lowercase();
                self.nodes
                    .keys()
                    .find(|k| k.to_lowercase() == lower)
                    .cloned()
            }
        }
    }

    pub fn query_neighbors(&self, start_entity: &str, max_depth: usize) -> (Vec<Node>, Vec<Edge>) {
        let mut visited_nodes = HashSet::new();
        let mut visited_edges = HashSet::new();

        let canonical_name = match self.find_canonical_node_name(start_entity) {
            Some(name) => name,
            None => return (Vec::new(), Vec::new()),
        };

        let mut current_level = vec![canonical_name.clone()];
        visited_nodes.insert(canonical_name);

        for _ in 0..max_depth {
            let mut next_level = Vec::new();
            for node in &current_level {
                for edge in &self.edges {
                    let neighbor_opt = match (edge.source == *node, edge.target == *node) {
                        (true, _) => Some(&edge.target),
                        (false, true) => Some(&edge.source),
                        (false, false) => None,
                    };
                    match neighbor_opt {
                        Some(neighbor) => {
                            match visited_nodes.insert(neighbor.clone()) {
                                true => next_level.push(neighbor.clone()),
                                false => {}
                            }
                            visited_edges.insert(edge.clone());
                        }
                        None => {}
                    }
                }
            }
            match next_level.is_empty() {
                true => break,
                false => current_level = next_level,
            }
        }

        let nodes = visited_nodes
            .into_iter()
            .filter_map(|name| self.nodes.get(&name).cloned())
            .collect();
        let edges = visited_edges.into_iter().collect();

        (nodes, edges)
    }

    pub fn extract_heuristics(&mut self, url: &str, title: &str, content: &str) {
        let triples = extract_heuristic_triples(url, title, content);
        match triples.is_empty() {
            true => {}
            false => {
                self.add_node(url.to_string(), "Document".to_string());
                for (source, rel, target) in triples {
                    self.add_edge(
                        source,
                        "Document".to_string(),
                        target,
                        "Technology/Topic".to_string(),
                        rel,
                    );
                }
            }
        }
    }

    pub fn prune_node(&mut self, name: &str) {
        let canonical_name = self
            .find_canonical_node_name(name)
            .unwrap_or_else(|| name.to_string());
        self.nodes.remove(&canonical_name);
        self.edges
            .retain(|e| e.source != canonical_name && e.target != canonical_name);
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_crud() {
        let mut graph = KnowledgeGraph::new();
        graph.add_edge(
            "Tokio".to_string(),
            "Library".to_string(),
            "Rust".to_string(),
            "Language".to_string(),
            "written_in".to_string(),
        );

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].relationship_type, "written_in");
    }

    #[test]
    fn test_graph_bfs_neighbors() {
        let mut graph = KnowledgeGraph::new();
        graph.add_edge(
            "A".into(),
            "Type".into(),
            "B".into(),
            "Type".into(),
            "link".into(),
        );
        graph.add_edge(
            "B".into(),
            "Type".into(),
            "C".into(),
            "Type".into(),
            "link".into(),
        );
        graph.add_edge(
            "C".into(),
            "Type".into(),
            "D".into(),
            "Type".into(),
            "link".into(),
        );

        let (nodes, edges) = graph.query_neighbors("B", 1);
        let mut names: Vec<String> = nodes.into_iter().map(|n| n.name).collect();
        names.sort();
        assert_eq!(
            names,
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
        assert_eq!(edges.len(), 2);

        let (nodes_ci, _) = graph.query_neighbors("b", 1);
        assert_eq!(nodes_ci.len(), 3);

        let (nodes_a, _) = graph.query_neighbors("A", 2);
        assert_eq!(nodes_a.len(), 3);
    }

    #[test]
    fn test_extract_heuristics() {
        let mut graph = KnowledgeGraph::new();
        graph.extract_heuristics(
            "https://example.com/rust-async",
            "Async Rust",
            "This article describes building async systems in Rust using the Tokio runtime.",
        );

        assert!(graph.nodes.contains_key("https://example.com/rust-async"));
        assert!(graph.nodes.contains_key("Rust"));
        assert!(graph.nodes.contains_key("Tokio"));

        let has_rust_edge = graph
            .edges
            .iter()
            .any(|e| e.source == "https://example.com/rust-async" && e.target == "Rust");
        assert!(has_rust_edge);
    }

    #[test]
    fn test_graph_pruning() {
        let mut graph = KnowledgeGraph::new();
        graph.add_edge(
            "Doc1".to_string(),
            "Document".to_string(),
            "Rust".to_string(),
            "Concept".to_string(),
            "mentions".to_string(),
        );
        assert_eq!(graph.nodes.len(), 2);

        graph.prune_node("Doc1");
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.edges.len(), 0);
    }
}
