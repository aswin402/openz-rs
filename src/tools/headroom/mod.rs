pub mod cache;
pub mod compress;
pub mod policy;
pub mod scoping;
pub mod stats;
pub mod syntax;

// Re-exports
pub use cache::{
    CacheAlignTool, CacheStatsTool, ClearCacheTool, ExportCacheTool, ImportCacheTool,
    SearchCacheTool,
};
pub use compress::{
    CompressContentTool, CompressDiffTool, CompressDirectoryTool, CompressFileTool,
    CompressSchemaTool, CompressUrlTool, RetrieveOriginalTool, RunAndCompressTool,
};
pub use scoping::{ScopeContextTool, SummarizeCodebaseTool};
pub use stats::{CountTokensTool, HeadroomStatsTool, HeadroomUsageTool, PingTool, ServerInfoTool};

// Shared Constants and Helpers
pub const MAX_INPUT_SIZE: usize = 512_000; // 500KB max input
pub const CACHE_CAPACITY: usize = 1000;

pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

pub(crate) fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub(crate) fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

pub fn is_binary_file(path: &std::path::Path) -> bool {
    match std::fs::read(path) {
        Ok(content) => content.iter().take(4096).any(|&b| b == 0x00),
        Err(_) => false,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DirTreeNode {
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) line_count: usize,
    pub(crate) files_count: usize,
    pub(crate) leaf_suffix: Option<String>,
    pub(crate) children: std::collections::BTreeMap<String, DirTreeNode>,
}

impl DirTreeNode {
    pub(crate) fn new_root(root_path: &std::path::Path) -> Self {
        Self {
            name: root_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(".")
                .to_string(),
            is_dir: true,
            line_count: 0,
            files_count: 0,
            leaf_suffix: None,
            children: std::collections::BTreeMap::new(),
        }
    }

    pub(crate) fn insert_rel_path(
        &mut self,
        rel_path: &std::path::Path,
        line_count: usize,
        leaf_suffix: String,
    ) {
        let parts: Vec<String> = rel_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        self.insert(&parts, line_count, leaf_suffix);
    }

    fn insert(&mut self, parts: &[String], line_count: usize, leaf_suffix: String) {
        match parts {
            [] => {}
            [only] => {
                let child = self
                    .children
                    .entry(only.clone())
                    .or_insert_with(|| DirTreeNode {
                        name: only.clone(),
                        is_dir: false,
                        line_count: 0,
                        files_count: 0,
                        leaf_suffix: None,
                        children: std::collections::BTreeMap::new(),
                    });
                child.line_count = line_count;
                child.files_count = 1;
                child.leaf_suffix = Some(leaf_suffix);
            }
            [first, rest @ ..] => {
                let child = self
                    .children
                    .entry(first.clone())
                    .or_insert_with(|| DirTreeNode {
                        name: first.clone(),
                        is_dir: true,
                        line_count: 0,
                        files_count: 0,
                        leaf_suffix: None,
                        children: std::collections::BTreeMap::new(),
                    });
                child.insert(rest, line_count, leaf_suffix);
            }
        }
    }

    pub(crate) fn update_counts(&mut self) {
        match self.is_dir {
            false => {}
            true => {
                self.files_count = 0;
                self.line_count = 0;
                for child in self.children.values_mut() {
                    child.update_counts();
                    self.files_count += child.files_count;
                    self.line_count += child.line_count;
                }
            }
        }
    }

    pub(crate) fn format_tree(
        &self,
        prefix: &str,
        is_last: bool,
        depth: usize,
        max_depth: usize,
        show_line_counts: bool,
    ) -> String {
        match depth > max_depth {
            true => {
                return match (self.files_count > 0, show_line_counts) {
                    (true, true) => format!(
                        "{}└── {} ({} files, {} lines)\n",
                        prefix, self.name, self.files_count, self.line_count
                    ),
                    (true, false) => {
                        format!("{}└── {} ({} files)\n", prefix, self.name, self.files_count)
                    }
                    (false, _) => String::new(),
                };
            }
            false => {}
        }
        let connector = match is_last {
            true => "└── ",
            false => "├── ",
        };
        let mut result = format!("{}{}{}", prefix, connector, self.name);
        match (&self.leaf_suffix, self.is_dir && depth > 0) {
            (Some(suffix), _) => result.push_str(suffix),
            (None, true) => result.push('/'),
            (None, false) => {}
        }
        result.push('\n');

        let child_prefix = match is_last {
            true => format!("{}    ", prefix),
            false => format!("{}│   ", prefix),
        };
        let children: Vec<&str> = self.children.keys().map(|k| k.as_str()).collect();
        for (i, child_name) in children.iter().enumerate() {
            match self.children.get(*child_name) {
                Some(child) => {
                    let is_last_child = i == children.len() - 1;
                    result.push_str(&child.format_tree(
                        &child_prefix,
                        is_last_child,
                        depth + 1,
                        max_depth,
                        show_line_counts,
                    ));
                }
                None => {}
            }
        }
        result
    }
}

#[cfg(test)]
mod tests;
