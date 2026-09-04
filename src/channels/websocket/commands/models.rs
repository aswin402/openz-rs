//! Model catalog and preference WebSocket response builders.

use serde_json::Value;

pub(crate) async fn models_list_event(
    config: &crate::config::schema::Config,
    envelope: &Value,
) -> Value {
    let requested_provider = envelope
        .get("provider")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut providers = Vec::new();
    for opt in crate::channels::configured_provider_model_options(config) {
        if requested_provider.is_some_and(|provider| provider != opt.name) {
            continue;
        }
        let models = if requested_provider.is_some() {
            crate::channels::resolved_provider_models_for_webui(&opt, config).await
        } else {
            crate::channels::preview_models_for_provider(&opt, config, 4)
        };
        providers.push(super::super::protocol::model_provider(
            opt.name,
            opt.display,
            models,
            opt.available,
            requested_provider.is_some(),
        ));
    }
    let prefs = crate::channels::load_model_prefs();
    super::super::protocol::models_list(
        providers,
        requested_provider.is_some(),
        prefs,
        config.agents.defaults.provider.clone(),
        config.agents.defaults.model.clone(),
    )
}

pub(crate) fn toggle_favorite_model_event(envelope: &Value) -> Value {
    let provider = envelope
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("");
    let model = envelope
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("");
    let prefs = crate::channels::toggle_favorite_model(provider, model);
    super::super::protocol::model_prefs(prefs)
}
