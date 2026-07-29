use crate::tools::{Tool, ToolMetadata, ToolRisk};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::PathBuf;

const INVENTORY_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct DeviceInventoryTool {
    path: PathBuf,
}

impl DeviceInventoryTool {
    pub fn new() -> Self {
        Self {
            path: crate::config::config_dir().join("device_inventory.json"),
        }
    }

    #[cfg(test)]
    fn with_path(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Default for DeviceInventoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceInventory {
    version: u32,
    capabilities: Vec<DeviceCapability>,
}

impl Default for DeviceInventory {
    fn default() -> Self {
        Self {
            version: INVENTORY_VERSION,
            capabilities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceCapability {
    id: String,
    category: String,
    name: String,
    command: String,
    args: Vec<String>,
    works_for: Vec<String>,
    paths: Vec<String>,
    notes: Option<String>,
    source: String,
    enabled: bool,
    success_count: u32,
    failure_count: u32,
    confidence: f64,
    created_at: String,
    updated_at: String,
    last_success: Option<String>,
    last_failure: Option<String>,
}

impl DeviceCapability {
    fn score_for(&self, category: Option<&str>, target_hint: Option<&str>) -> f64 {
        if !self.enabled {
            return -1.0;
        }

        let mut score = self.confidence.clamp(0.0, 1.0) * 100.0;
        if category
            .map(|cat| self.category.eq_ignore_ascii_case(cat))
            .unwrap_or(false)
        {
            score += 35.0;
        }

        if target_hint
            .map(|hint| {
                self.works_for
                    .iter()
                    .any(|entry| entry.eq_ignore_ascii_case(hint))
            })
            .unwrap_or(false)
        {
            score += 25.0;
        }

        score + f64::from(self.success_count) * 3.0 - f64::from(self.failure_count) * 4.0
    }
}

pub(crate) fn record_successful_default_open(target: &str) -> Result<Option<String>> {
    let path = crate::config::config_dir().join("device_inventory.json");
    record_successful_default_open_at(path, target)
}

fn record_successful_default_open_at(path: PathBuf, target: &str) -> Result<Option<String>> {
    let Some((category, hint)) = classify_target(target) else {
        return Ok(None);
    };

    let tool = DeviceInventoryTool { path };
    let mut inventory = tool.load_inventory()?;
    let id = format!("system-default-{}-{}", category, normalize_token(&hint));
    let now = now_timestamp();

    if let Some(entry) = inventory
        .capabilities
        .iter_mut()
        .find(|entry| entry.id == id)
    {
        if !entry
            .works_for
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&hint))
        {
            entry.works_for.push(hint.clone());
            entry.works_for = normalize_list(std::mem::take(&mut entry.works_for));
        }
        entry.success_count = entry.success_count.saturating_add(1);
        entry.last_success = Some(now.clone());
        entry.updated_at = now;
        entry.confidence = confidence(entry.success_count, entry.failure_count);
    } else {
        inventory.capabilities.push(DeviceCapability {
            id: id.clone(),
            category,
            name: "System default opener".to_string(),
            command: "system-default".to_string(),
            args: vec!["{target}".to_string()],
            works_for: vec![hint],
            paths: Vec::new(),
            notes: Some("Recorded automatically after open_path succeeded.".to_string()),
            source: "successful_open".to_string(),
            enabled: true,
            success_count: 1,
            failure_count: 0,
            confidence: confidence(1, 0),
            created_at: now.clone(),
            updated_at: now.clone(),
            last_success: Some(now),
            last_failure: None,
        });
    }

    tool.save_inventory(&inventory)?;
    Ok(Some(id))
}

fn classify_target(target: &str) -> Option<(String, String)> {
    let hint = target_hint(target)?;
    let category = if hint == "url" {
        "browser"
    } else {
        match hint.as_str() {
            ".png" | ".jpg" | ".jpeg" | ".webp" | ".gif" | ".bmp" | ".svg" => "image_viewer",
            ".mp4" | ".mkv" | ".webm" | ".mov" | ".avi" => "video_player",
            ".mp3" | ".wav" | ".ogg" | ".flac" | ".m4a" => "audio_player",
            ".pdf" => "pdf_viewer",
            ".doc" | ".docx" | ".xls" | ".xlsx" | ".ppt" | ".pptx" | ".odt" | ".ods" | ".odp" => {
                "office_docs"
            }
            ".txt" | ".md" | ".rs" | ".js" | ".ts" | ".html" | ".css" | ".json" | ".toml"
            | ".yaml" | ".yml" => "editor",
            _ => "file_manager",
        }
    };
    Some((category.to_string(), hint))
}

#[async_trait::async_trait]
impl Tool for DeviceInventoryTool {
    fn name(&self) -> &str {
        "device_inventory"
    }

    fn description(&self) -> &str {
        "Maintain a local device/app capability registry for the user's computer. Store GUI apps, viewers, editors, browser choices, file paths, and what worked so OpenZ can suggest the best known way to open or handle local targets without rediscovering every time."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "add", "update", "delete", "record_success", "record_failure", "suggest"],
                    "description": "Registry operation to perform."
                },
                "id": {
                    "type": "string",
                    "description": "Capability id for get/update/delete/record_success/record_failure."
                },
                "category": {
                    "type": "string",
                    "description": "Capability category, such as image_viewer, video_player, browser, editor, terminal, file_manager, pdf_viewer, audio_player, office_docs, dev_server, or custom_app."
                },
                "name": {
                    "type": "string",
                    "description": "Human-readable app or capability name."
                },
                "command": {
                    "type": "string",
                    "description": "Executable or command name to remember. This tool stores it only; execution is handled by safer open/command tools."
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command argument template, using {path} or {url} placeholders when needed."
                },
                "works_for": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Extensions, MIME types, or target hints this capability works for, for example .png, image/png, .mp4, URL."
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Useful local paths related to this capability, such as config paths, app paths, or project folders."
                },
                "notes": {
                    "type": "string",
                    "description": "Short non-secret notes about the capability."
                },
                "source": {
                    "type": "string",
                    "description": "Where this entry came from: user, auto_discovered, successful_open, manual_fix."
                },
                "enabled": {
                    "type": "boolean",
                    "description": "Whether this capability should be suggested."
                },
                "target": {
                    "type": "string",
                    "description": "Target path, URL, extension, or MIME hint used by suggest to rank matching capabilities."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 20,
                    "description": "Maximum suggestion count. Defaults to 5."
                }
            },
            "required": ["action"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            domain: "self_management",
            risk: ToolRisk::Medium,
            uses_network: false,
            writes_disk: true,
            spawns_process: false,
            requires_approval: false,
            priority: 88,
            aliases: &[
                "device apps",
                "local capability registry",
                "remember viewer",
                "app inventory",
            ],
            examples: &[
                "Suggest an image_viewer for .png",
                "Record that Firefox worked for opening a generated image",
            ],
            when_to_use: "Use before opening/showing/playing local files when OpenZ should reuse known apps, viewers, editors, or paths for this computer.",
            when_not_to_use: "Avoid storing secrets, API keys, arbitrary shell pipelines, or one-off commands that should not be reused.",
            recommended_timeout_secs: None,
        }
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing 'action' parameter"))?;

        let mut inventory = self.load_inventory()?;
        match action {
            "list" => self.list(&inventory, arguments),
            "get" => self.get(&inventory, arguments),
            "add" => self.add(&mut inventory, arguments),
            "update" => self.update(&mut inventory, arguments),
            "delete" => self.delete(&mut inventory, arguments),
            "record_success" => self.record_outcome(&mut inventory, arguments, true),
            "record_failure" => self.record_outcome(&mut inventory, arguments, false),
            "suggest" => self.suggest(&inventory, arguments),
            other => Err(anyhow!("Unsupported device_inventory action: {other}")),
        }
    }
}

impl DeviceInventoryTool {
    fn load_inventory(&self) -> Result<DeviceInventory> {
        if !self.path.exists() {
            return Ok(DeviceInventory::default());
        }
        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read {}", self.path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", self.path.display()))
    }

    fn save_inventory(&self, inventory: &DeviceInventory) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }

        let tmp_path = self
            .path
            .with_extension(format!("json.tmp.{}", uuid::Uuid::new_v4()));
        let content = serde_json::to_string_pretty(inventory)?;
        std::fs::write(&tmp_path, content)
            .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, &self.path)
            .with_context(|| format!("Failed to replace {}", self.path.display()))?;
        Ok(())
    }

    fn list(&self, inventory: &DeviceInventory, arguments: &Value) -> Result<Value> {
        let category = arguments.get("category").and_then(Value::as_str);
        let enabled_only = arguments
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let capabilities: Vec<_> = inventory
            .capabilities
            .iter()
            .filter(|entry| {
                category
                    .map(|cat| entry.category.eq_ignore_ascii_case(cat))
                    .unwrap_or(true)
                    && (!enabled_only || entry.enabled)
            })
            .collect();

        Ok(json!({
            "status": "success",
            "count": capabilities.len(),
            "capabilities": capabilities,
            "storage_path": self.path.to_string_lossy().to_string(),
        }))
    }

    fn get(&self, inventory: &DeviceInventory, arguments: &Value) -> Result<Value> {
        let id = required_str(arguments, "id")?;
        let entry = inventory
            .capabilities
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| anyhow!("Capability not found: {id}"))?;

        Ok(json!({
            "status": "success",
            "capability": entry,
        }))
    }

    fn add(&self, inventory: &mut DeviceInventory, arguments: &Value) -> Result<Value> {
        let category = normalize_token(required_str(arguments, "category")?);
        let name = required_str(arguments, "name")?.trim().to_string();
        let command = required_str(arguments, "command")?.trim().to_string();
        validate_command(&command)?;

        let id = arguments
            .get("id")
            .and_then(Value::as_str)
            .map(normalize_token)
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| format!("{}-{}", category, uuid::Uuid::new_v4()));

        if inventory.capabilities.iter().any(|entry| entry.id == id) {
            return Err(anyhow!("Capability already exists: {id}"));
        }

        let now = now_timestamp();
        let entry = DeviceCapability {
            id: id.clone(),
            category,
            name,
            command,
            args: string_array(arguments, "args"),
            works_for: normalize_list(string_array(arguments, "works_for")),
            paths: string_array(arguments, "paths"),
            notes: arguments
                .get("notes")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            source: arguments
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("user")
                .trim()
                .to_string(),
            enabled: arguments
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            success_count: 0,
            failure_count: 0,
            confidence: 0.5,
            created_at: now.clone(),
            updated_at: now,
            last_success: None,
            last_failure: None,
        };

        inventory.capabilities.push(entry.clone());
        self.save_inventory(inventory)?;
        Ok(json!({
            "status": "success",
            "capability": entry,
            "storage_path": self.path.to_string_lossy().to_string(),
        }))
    }

    fn update(&self, inventory: &mut DeviceInventory, arguments: &Value) -> Result<Value> {
        let id = required_str(arguments, "id")?;
        let entry = find_entry_mut(inventory, id)?;

        if let Some(category) = arguments.get("category").and_then(Value::as_str) {
            entry.category = normalize_token(category);
        }
        if let Some(name) = arguments.get("name").and_then(Value::as_str) {
            entry.name = name.trim().to_string();
        }
        if let Some(command) = arguments.get("command").and_then(Value::as_str) {
            validate_command(command)?;
            entry.command = command.trim().to_string();
        }
        if arguments.get("args").is_some() {
            entry.args = string_array(arguments, "args");
        }
        if arguments.get("works_for").is_some() {
            entry.works_for = normalize_list(string_array(arguments, "works_for"));
        }
        if arguments.get("paths").is_some() {
            entry.paths = string_array(arguments, "paths");
        }
        if let Some(notes) = arguments.get("notes").and_then(Value::as_str) {
            entry.notes = Some(notes.trim().to_string()).filter(|s| !s.is_empty());
        }
        if let Some(source) = arguments.get("source").and_then(Value::as_str) {
            entry.source = source.trim().to_string();
        }
        if let Some(enabled) = arguments.get("enabled").and_then(Value::as_bool) {
            entry.enabled = enabled;
        }
        entry.updated_at = now_timestamp();
        let updated = entry.clone();

        self.save_inventory(inventory)?;
        Ok(json!({
            "status": "success",
            "capability": updated,
        }))
    }

    fn delete(&self, inventory: &mut DeviceInventory, arguments: &Value) -> Result<Value> {
        let id = required_str(arguments, "id")?;
        let before = inventory.capabilities.len();
        inventory.capabilities.retain(|entry| entry.id != id);
        let deleted = before != inventory.capabilities.len();
        if !deleted {
            return Err(anyhow!("Capability not found: {id}"));
        }

        self.save_inventory(inventory)?;
        Ok(json!({
            "status": "success",
            "deleted": id,
        }))
    }

    fn record_outcome(
        &self,
        inventory: &mut DeviceInventory,
        arguments: &Value,
        success: bool,
    ) -> Result<Value> {
        let id = required_str(arguments, "id")?;
        let entry = find_entry_mut(inventory, id)?;
        let now = now_timestamp();
        if success {
            entry.success_count = entry.success_count.saturating_add(1);
            entry.last_success = Some(now.clone());
        } else {
            entry.failure_count = entry.failure_count.saturating_add(1);
            entry.last_failure = Some(now.clone());
        }
        entry.updated_at = now;
        entry.confidence = confidence(entry.success_count, entry.failure_count);
        let updated = entry.clone();

        self.save_inventory(inventory)?;
        Ok(json!({
            "status": "success",
            "capability": updated,
        }))
    }

    fn suggest(&self, inventory: &DeviceInventory, arguments: &Value) -> Result<Value> {
        let category = arguments.get("category").and_then(Value::as_str);
        let target_hint = arguments
            .get("target")
            .and_then(Value::as_str)
            .and_then(target_hint);
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 20) as usize;

        let mut scored: Vec<_> = inventory
            .capabilities
            .iter()
            .map(|entry| (entry.score_for(category, target_hint.as_deref()), entry))
            .filter(|(score, _)| *score >= 0.0)
            .collect();

        scored.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .partial_cmp(left_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.name.cmp(&right.name))
        });

        let suggestions: Vec<_> = scored
            .into_iter()
            .take(limit)
            .map(|(score, entry)| {
                json!({
                    "score": score,
                    "capability": entry,
                })
            })
            .collect();

        Ok(json!({
            "status": "success",
            "count": suggestions.len(),
            "target_hint": target_hint,
            "suggestions": suggestions,
        }))
    }
}

fn find_entry_mut<'a>(
    inventory: &'a mut DeviceInventory,
    id: &str,
) -> Result<&'a mut DeviceCapability> {
    inventory
        .capabilities
        .iter_mut()
        .find(|entry| entry.id == id)
        .ok_or_else(|| anyhow!("Capability not found: {id}"))
}

fn required_str<'a>(arguments: &'a Value, field: &str) -> Result<&'a str> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Missing '{field}' parameter"))
}

fn string_array(arguments: &Value, field: &str) -> Vec<String> {
    arguments
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_list(items: Vec<String>) -> Vec<String> {
    let mut seen = BTreeMap::new();
    for item in items {
        seen.insert(item.trim().to_ascii_lowercase(), item.trim().to_string());
    }
    seen.into_values().collect()
}

fn normalize_token(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn validate_command(command: &str) -> Result<()> {
    if command.chars().any(|c| matches!(c, '\n' | '\r' | '\0')) {
        return Err(anyhow!("Command must be a single executable name or path"));
    }
    Ok(())
}

fn target_hint(target: &str) -> Option<String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some("url".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return std::path::Path::new(trimmed)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{}", ext.to_ascii_lowercase()));
    }
    Some(trimmed.to_ascii_lowercase())
}

fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn confidence(success_count: u32, failure_count: u32) -> f64 {
    let successes = f64::from(success_count);
    let failures = f64::from(failure_count);
    ((successes + 1.0) / (successes + failures + 2.0)).clamp(0.05, 0.99)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_tool() -> DeviceInventoryTool {
        let path = std::env::temp_dir().join(format!(
            "openz_device_inventory_{}.json",
            uuid::Uuid::new_v4()
        ));
        DeviceInventoryTool::with_path(path)
    }

    #[tokio::test]
    async fn record_successful_default_open_learns_target_type() -> Result<()> {
        let path = std::env::temp_dir().join(format!(
            "openz_device_inventory_open_{}.json",
            uuid::Uuid::new_v4()
        ));

        let id = record_successful_default_open_at(path.clone(), "/tmp/render.png")?;
        assert_eq!(id.as_deref(), Some("system-default-image_viewer-png"));

        let tool = DeviceInventoryTool::with_path(path);
        let result = tool
            .call(&json!({
                "action": "suggest",
                "category": "image_viewer",
                "target": "/tmp/another.png"
            }))
            .await?;

        assert_eq!(
            result["suggestions"][0]["capability"]["command"],
            "system-default"
        );
        assert_eq!(result["suggestions"][0]["capability"]["success_count"], 1);
        Ok(())
    }

    #[tokio::test]
    async fn suggest_prefers_successful_matching_capability() -> Result<()> {
        let tool = test_tool();

        tool.call(&json!({
            "action": "add",
            "id": "firefox-image",
            "category": "image_viewer",
            "name": "Firefox",
            "command": "firefox",
            "args": ["{path}"],
            "works_for": [".png", ".jpg"]
        }))
        .await?;
        tool.call(&json!({
            "action": "add",
            "id": "vlc-video",
            "category": "video_player",
            "name": "VLC",
            "command": "vlc",
            "args": ["{path}"],
            "works_for": [".mp4"]
        }))
        .await?;
        tool.call(&json!({
            "action": "record_success",
            "id": "firefox-image"
        }))
        .await?;

        let result = tool
            .call(&json!({
                "action": "suggest",
                "category": "image_viewer",
                "target": "/tmp/render.png"
            }))
            .await?;

        assert_eq!(result["status"], "success");
        assert_eq!(
            result["suggestions"][0]["capability"]["id"],
            "firefox-image"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_and_delete_capability() -> Result<()> {
        let tool = test_tool();
        tool.call(&json!({
            "action": "add",
            "id": "viewer",
            "category": "image_viewer",
            "name": "Viewer",
            "command": "xdg-open"
        }))
        .await?;

        let updated = tool
            .call(&json!({
                "action": "update",
                "id": "viewer",
                "works_for": [".webp"],
                "enabled": false
            }))
            .await?;
        assert_eq!(updated["capability"]["enabled"], false);
        assert_eq!(updated["capability"]["works_for"][0], ".webp");

        let deleted = tool
            .call(&json!({
                "action": "delete",
                "id": "viewer"
            }))
            .await?;
        assert_eq!(deleted["deleted"], "viewer");
        Ok(())
    }
}
