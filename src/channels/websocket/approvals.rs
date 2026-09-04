//! Ownership and lifecycle state for WebSocket security approvals.
//!
//! Approval requests are bound to both the authenticated client and chat. A
//! disconnect removes those requests and resolves them as denied, preserving
//! the gateway's fail-closed behavior.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WsApprovalContext {
    pub client_id: String,
    pub chat_id: String,
}

struct PendingWsApproval {
    context: WsApprovalContext,
    tx: tokio::sync::oneshot::Sender<bool>,
}

tokio::task_local! {
    static WS_APPROVAL_CONTEXT: Option<WsApprovalContext>;
}

pub fn current_ws_approval_context() -> Option<WsApprovalContext> {
    WS_APPROVAL_CONTEXT
        .try_with(|context| context.clone())
        .ok()
        .flatten()
}

pub async fn with_ws_approval_context<F, T>(context: WsApprovalContext, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    WS_APPROVAL_CONTEXT.scope(Some(context), future).await
}

static WS_APPROVALS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, PendingWsApproval>>,
> = std::sync::OnceLock::new();

/// Register a pending security-approval request for a WebSocket client.
pub fn register_ws_approval(
    req_id: String,
    context: WsApprovalContext,
    tx: tokio::sync::oneshot::Sender<bool>,
) {
    let map = WS_APPROVALS.get_or_init(std::sync::Mutex::default);
    if let Ok(mut approvals) = map.lock() {
        approvals.insert(req_id, PendingWsApproval { context, tx });
    }
}

/// Resolve a pending security-approval request from a client response.
pub fn resolve_ws_approval(req_id: &str, client_id: &str, chat_id: &str, approved: bool) -> bool {
    let map = WS_APPROVALS.get_or_init(std::sync::Mutex::default);
    let pending = map.lock().ok().and_then(|mut approvals| {
        let matches = approvals.get(req_id).is_some_and(|pending| {
            pending.context.client_id == client_id && pending.context.chat_id == chat_id
        });
        if matches {
            approvals.remove(req_id)
        } else {
            None
        }
    });

    pending
        .map(|pending| {
            let _ = pending.tx.send(approved);
            true
        })
        .unwrap_or(false)
}

/// Deny and remove all pending approvals owned by a disconnected client.
pub fn cancel_ws_approvals_for_client(client_id: &str) {
    let map = WS_APPROVALS.get_or_init(std::sync::Mutex::default);
    let pending = map
        .lock()
        .ok()
        .map(|mut approvals| {
            let req_ids = approvals
                .iter()
                .filter(|(_, pending)| pending.context.client_id == client_id)
                .map(|(req_id, _)| req_id.clone())
                .collect::<Vec<_>>();
            req_ids
                .into_iter()
                .filter_map(|req_id| approvals.remove(&req_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for pending in pending {
        let _ = pending.tx.send(false);
    }
}
