use crate::providers::ToolCallRequest;
use crate::session::Message;
use std::io::Write;

pub(crate) struct ToolApprovalDecision {
    pub parse_error: Option<String>,
    pub repeat_count: usize,
    pub is_loop: bool,
    pub forbidden: bool,
    pub approved: bool,
    pub approval_requested: bool,
    pub should_halt: bool,
}

pub(crate) async fn evaluate_tool_approval(
    call: &ToolCallRequest,
    messages: &[Message],
    session_key: &str,
    security_mode: &str,
    silent: bool,
    loop_blocked_count: &mut usize,
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
    let is_loop = repeat_count >= repeat_threshold;
    let mut should_halt = false;
    match is_loop && parse_error.is_none() {
        true => {
            *loop_blocked_count += 1;
            should_halt = *loop_blocked_count >= 3;
        }
        false => {}
    }

    let is_forbidden = parse_error.is_none()
        && crate::agent::security::SecurityGuard::is_forbidden(&call.name, &call.arguments);
    let needs_approval = parse_error.is_none()
        && !is_forbidden
        && !is_loop
        && crate::agent::security::SecurityGuard::is_sensitive_with_mode(
            &call.name,
            &call.arguments,
            security_mode,
        );

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
        forbidden,
        approved,
        approval_requested,
        should_halt,
    }
}
