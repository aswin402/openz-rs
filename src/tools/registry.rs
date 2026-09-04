//! Registry mechanics shared by the public [`super::ToolRegistry`] facade.
//!
//! The registry facade still owns context-aware dynamic tool construction.
//! This module isolates the name/index operations so alias resolution and
//! duplicate handling do not get mixed with provider or subagent wiring.

use super::{normalize_tool_name, tool_spec, Tool};
#[cfg(test)]
use super::all_tool_specs;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::sync::Arc;

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct StaticToolDriftReport {
    pub(crate) missing_registered: Vec<String>,
    pub(crate) noncanonical_registered: Vec<String>,
}

#[cfg(test)]
impl StaticToolDriftReport {
    pub(crate) fn is_clean(&self) -> bool {
        self.missing_registered.is_empty() && self.noncanonical_registered.is_empty()
    }
}

fn identity_keys(name: &str) -> Vec<String> {
    let canonical = normalize_tool_name(name);
    let mut keys = vec![canonical.clone()];
    if let Some(spec) = tool_spec(&canonical) {
        keys.extend(
            spec.aliases
                .iter()
                .map(|alias| normalize_tool_name(alias)),
        );
    }
    keys.sort();
    keys.dedup();
    keys
}

pub(crate) fn insert_unique_tool(
    tools: &mut HashMap<String, Arc<dyn Tool>>,
    tool: Arc<dyn Tool>,
) -> bool {
    let name = tool.name().to_string();
    let new_keys = identity_keys(&name);
    if let Some(existing) = tools.values().find(|existing| {
        identity_keys(existing.name())
            .iter()
            .any(|key| new_keys.iter().any(|new_key| new_key == key))
    }) {
        tracing::error!(
            tool = %name,
            existing_tool = %existing.name(),
            "Ignoring tool registration with a conflicting canonical name or alias"
        );
        return false;
    }
    tools.insert(name, tool);
    true
}

pub(crate) fn resolve_static_name(
    tools: &HashMap<String, Arc<dyn Tool>>,
    requested: &str,
) -> Option<String> {
    let requested = normalize_tool_name(requested);
    if requested.is_empty() {
        return None;
    }

    let mut matches = tools
        .values()
        .filter_map(|tool| {
            let canonical = normalize_tool_name(tool.name());
            // Only curated aliases are executable compatibility names.
            // Broad inferred labels (for example, "delegate" for subagents)
            // are deliberately excluded because they are ambiguous.
            let aliases = tool_spec(&canonical)
                .map(|spec| spec.aliases)
                .unwrap_or(&[]);
            if canonical == requested
                || aliases
                    .iter()
                    .any(|alias| normalize_tool_name(alias) == requested)
            {
                Some(tool.name().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    (matches.len() == 1).then(|| matches.remove(0))
}

/// Compare curated metadata definitions with the tools currently registered.
/// Dynamic integrations are intentionally ignored: only names resolvable by
/// `tool_spec` participate in the reverse check.
#[cfg(test)]
pub(crate) fn static_tool_drift(
    tools: &HashMap<String, Arc<dyn Tool>>,
) -> StaticToolDriftReport {
    let registered_curated_names: BTreeSet<String> = tools
        .values()
        .filter_map(|tool| tool_spec(tool.name()).map(|spec| spec.name.to_string()))
        .collect();

    let mut report = StaticToolDriftReport {
        missing_registered: all_tool_specs()
            .iter()
            .filter(|spec| !registered_curated_names.contains(spec.name))
            .map(|spec| spec.name.to_string())
            .collect(),
        noncanonical_registered: tools
            .values()
            .filter_map(|tool| {
                let spec = tool_spec(tool.name())?;
                (tool.name() != spec.name).then(|| {
                    format!(
                        "{} is registered for curated tool '{}'",
                        tool.name(),
                        spec.name
                    )
                })
            })
            .collect(),
    };
    report.noncanonical_registered.sort();
    report
}
