use crate::providers::ToolCallRequest;
use crate::session::Message;
use std::io::Write;

pub(crate) const MAX_CONSECUTIVE_LOOP_BLOCKS: usize = 4;
pub(crate) const MAX_TOTAL_LOOP_BLOCKS: usize = 12;

/// Self-healing loop escalation tracker.
/// Resets `consecutive_blocks` to 0 whenever any non-blocked tool executes,
/// preventing scattered repeat nudges across a 200-step long-horizon turn
/// from prematurely halting execution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LoopEscalationState {
    pub consecutive_blocks: usize,
    pub total_blocks: usize,
}

impl LoopEscalationState {
    pub(crate) fn record_loop_block(&mut self) -> bool {
        self.consecutive_blocks += 1;
        self.total_blocks += 1;
        self.consecutive_blocks >= MAX_CONSECUTIVE_LOOP_BLOCKS
            || self.total_blocks >= MAX_TOTAL_LOOP_BLOCKS
    }

    pub(crate) fn record_forward_progress(&mut self) {
        self.consecutive_blocks = 0;
    }
}

pub(crate) struct ToolApprovalDecision {
    pub parse_error: Option<String>,
    pub repeat_count: usize,
    pub is_loop: bool,
    pub loop_message: Option<String>,
    pub loop_cycle: Option<super::loop_control::TrajectoryCycle>,
    pub forbidden: bool,
    pub forbidden_reason: Option<&'static str>,
    pub approved: bool,
    pub unattended_denied: bool,
    pub approval_requested: bool,
    pub should_halt: bool,
}

pub(crate) async fn evaluate_tool_approval(
    call: &ToolCallRequest,
    messages: &[Message],
    session_key: &str,
    security_mode: &str,
    silent: bool,
    loop_state: &mut LoopEscalationState,
) -> ToolApprovalDecision {
    let parse_error = call
        .arguments
        .get("parse_error")
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    let repeat_count =
        super::loop_control::count_previous_tool_calls(messages, &call.name, &call.arguments);
    let repeat_threshold =
        super::loop_control::tool_repetition_block_threshold(&call.name, &call.arguments);
    let single_repeat_loop = repeat_count >= repeat_threshold;
    let periodic_cycle = (!single_repeat_loop)
        .then(|| {
            super::loop_control::detect_trajectory_cycle(messages, &call.name, &call.arguments)
        })
        .flatten();
    let is_loop = single_repeat_loop || periodic_cycle.is_some();

    let loop_message = match (single_repeat_loop, periodic_cycle.as_ref()) {
        (true, _) => Some(format!(
            "Loop detected: You have already executed the tool '{}' with these exact arguments {} times in this active window without intervening state changes. Execution was skipped. Analyze previous outputs, vary parameters, or pivot to a different strategy.",
            call.name, repeat_count
        )),
        (false, Some(cycle)) => Some(cycle.format_steering_error(&call.name)),
        (false, None) => None,
    };

    let forbidden_reason = parse_error.is_none().then(|| {
        crate::agent::security::SecurityGuard::forbidden_reason_for_session(
            session_key,
            &call.name,
            &call.arguments,
        )
    }).flatten();
    let is_forbidden = forbidden_reason.is_some();

    let mut should_halt = false;
    match (parse_error.is_none(), is_loop, is_forbidden) {
        (true, true, _) => {
            should_halt = loop_state.record_loop_block();
        }
        (true, false, false) => {
            loop_state.record_forward_progress();
        }
        _ => {}
    }

    let needs_approval = parse_error.is_none()
        && !is_forbidden
        && !is_loop
        && crate::agent::security::SecurityGuard::is_sensitive_with_mode(
            &call.name,
            &call.arguments,
            security_mode,
        );

    let unattended = needs_approval
        && crate::agent::security::SecurityGuard::is_unattended_session(session_key);

    let (forbidden, approved, approval_requested) = match (is_forbidden, needs_approval) {
        (true, _) => (true, true, false),
        (false, true) => {
            match silent {
                true => {}
                false => {
                    print!("\r\x1b[2K");
                    let _ = std::io::stdout().flush();
                }
            }
            let user_approved =
                crate::agent::security::ask_approval(session_key, &call.name, &call.arguments)
                    .await
                    .unwrap_or(false);
            (false, user_approved, true)
        }
        (false, false) => (false, true, false),
    };

    ToolApprovalDecision {
        parse_error,
        repeat_count,
        is_loop,
        loop_message,
        loop_cycle: periodic_cycle,
        forbidden,
        forbidden_reason,
        approved,
        unattended_denied: unattended && !approved,
        approval_requested,
        should_halt,
    }
}
