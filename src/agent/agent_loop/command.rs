use anyhow::Result;
use super::{AgentLoop, TurnContext, TurnState};

pub async fn handle(_loop_ref: &AgentLoop, _ctx: &mut TurnContext<'_>) -> Result<TurnState> {
    Ok(TurnState::Done)
}
