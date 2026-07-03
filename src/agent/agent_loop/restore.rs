use anyhow::{Result, anyhow};
use super::{AgentLoop, TurnContext, TurnState};

pub async fn handle(loop_ref: &AgentLoop, ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    ctx.session_file_lock = Some(loop_ref.session_manager.acquire_lock_async(ctx.session_key).await.map_err(|e| {
        anyhow!("Cannot start session: {}", e)
    })?);
    ctx.session = loop_ref.session_manager.get_or_create_async(ctx.session_key).await;
    if !ctx.user_content.starts_with('/') {
        ctx.interaction_id = crate::tools::shared_memory::log_interaction(ctx.session_key, ctx.user_content).await.ok();
    }
    ctx.session.add_message("user", ctx.user_content);
    tracing::info!(session = %ctx.session_key, "Restored session history ({} messages). User prompt: {:?}", ctx.session.messages.len(), ctx.user_content);

    let parts = crate::providers::parse_multimodal_content(ctx.user_content).await;
    let has_images = parts.iter().any(|p| matches!(p, crate::providers::ContentPart::Image { .. }));
    let supports_vision = crate::providers::model_supports_vision(&loop_ref.config.agents.defaults.model);
    let silent = crate::agent::style::spinner::is_silent();
    if has_images && !supports_vision && !silent {
        eprintln!("{}▲ Image unsupported: The active model '{}' does not support images. Images will be ignored.{}", crate::agent::style::AURA_GOLD, loop_ref.config.agents.defaults.model, crate::agent::style::COLOR_RESET);
    }

    if let Err(e) = loop_ref.session_manager.save(&ctx.session).await {
        tracing::warn!("Failed to save session incrementally in Restore: {}", e);
    }

    Ok(TurnState::Compact)
}
