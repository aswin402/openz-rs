//! Skill and subagent profile WebSocket command handlers.

use serde_json::{json, Value};

pub(crate) fn subagent_profile_events() -> Vec<Value> {
    crate::subagents::load_profiles()
        .unwrap_or_default()
        .into_iter()
        .map(|profile| {
            let is_core = crate::subagents::is_default_subagent(&profile.name);
            let model = profile.model.unwrap_or_else(|| "default".to_string());
            let provider = model
                .split_once('/')
                .map(|(provider, _)| provider.to_string())
                .unwrap_or_else(|| "auto".to_string());
            json!({
                "name": profile.name,
                "description": profile.description,
                "systemPrompt": profile.system_prompt,
                "model": model,
                "provider": provider,
                "fallbacks": profile.fallbacks.unwrap_or_default(),
                "isCore": is_core,
                "isProtected": is_core,
                "source": if is_core { "core" } else { "user" },
                "fallbackLimit": crate::subagents::MAX_SUBAGENT_FALLBACKS,
            })
        })
        .collect()
}

pub(crate) fn save_skill_event(envelope: &Value) -> Value {
    let name = envelope
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let content = envelope
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("");
    if name.is_empty() {
        super::super::protocol::event_error("Skill name is required.")
    } else {
        match crate::agent::skills::save_skill(name, content) {
            Ok(()) => super::super::protocol::skills_updated(
                crate::agent::skills::load_skill_views().unwrap_or_default(),
                "saved",
                Some(name.to_string()),
            ),
            Err(err) => super::super::protocol::event_error(format!(
                "Failed to save skill: {}",
                err
            )),
        }
    }
}

pub(crate) fn delete_skill_event(envelope: &Value) -> Value {
    let name = envelope
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        super::super::protocol::event_error("Skill name is required.")
    } else {
        match crate::agent::skills::delete_skill(name) {
            Ok(()) => super::super::protocol::skills_updated(
                crate::agent::skills::load_skill_views().unwrap_or_default(),
                "deleted",
                Some(name.to_string()),
            ),
            Err(err) => super::super::protocol::event_error(format!(
                "Failed to delete skill: {}",
                err
            )),
        }
    }
}

pub(crate) async fn save_subagent_event(
    config: &crate::config::schema::Config,
    envelope: &Value,
) -> Value {
    let name = envelope
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let description = envelope
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let system_prompt = envelope
        .get("systemPrompt")
        .or_else(|| envelope.get("system_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let model = envelope
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "default");
    let fallbacks = envelope
        .get("fallbacks")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if name.is_empty() || description.is_empty() || system_prompt.is_empty() {
        return super::super::protocol::event_error(
            "Subagent name, description, and system prompt are required.",
        );
    }

    let args = json!({
        "name": name,
        "description": description,
        "system_prompt": system_prompt,
        "model": model,
        "fallbacks": fallbacks,
    });
    let tool = crate::tools::subagent::CreateSubagentTool {
        config: config.clone(),
    };
    match crate::tools::Tool::call(&tool, &args).await {
        Ok(_) => super::super::protocol::subagents_updated(
            subagent_profile_events(),
            "saved",
            Some(name.to_string()),
        ),
        Err(err) => super::super::protocol::event_error(format!(
            "Failed to save subagent: {}",
            err
        )),
    }
}

pub(crate) async fn update_subagent_settings_event(
    config: &crate::config::schema::Config,
    envelope: &Value,
) -> Value {
    let name = envelope
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        return super::super::protocol::event_error("Subagent name is required.");
    }

    let mut args = json!({ "name": name });
    if let Some(value) = envelope.get("model") {
        args["model"] = value.clone();
    }
    if let Some(value) = envelope.get("fallbacks") {
        args["fallbacks"] = value.clone();
    }
    let tool = crate::tools::subagent::UpdateSubagentSettingsTool {
        config: config.clone(),
    };
    match crate::tools::Tool::call(&tool, &args).await {
        Ok(_) => super::super::protocol::subagents_updated(
            subagent_profile_events(),
            "settings_updated",
            Some(name.to_string()),
        ),
        Err(err) => super::super::protocol::event_error(format!(
            "Failed to update subagent settings: {}",
            err
        )),
    }
}

pub(crate) async fn delete_subagent_event(envelope: &Value) -> Value {
    let name = envelope
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        return super::super::protocol::event_error("Subagent name is required.");
    }

    let args = json!({ "name": name });
    let tool = crate::tools::subagent::DeleteSubagentTool;
    match crate::tools::Tool::call(&tool, &args).await {
        Ok(_) => super::super::protocol::subagents_updated(
            subagent_profile_events(),
            "deleted",
            Some(name.to_string()),
        ),
        Err(err) => super::super::protocol::event_error(format!(
            "Failed to delete subagent: {}",
            err
        )),
    }
}
