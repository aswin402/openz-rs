use super::*;

#[tokio::test]
async fn websocket_turn_stop_is_scoped_to_owner() {
    let turn_id = format!("turn-test-{}", uuid::Uuid::new_v4());
    let token = crate::tools::subagent::CancellationToken::new();
    register_ws_turn(
        turn_id.clone(),
        "client-a".to_string(),
        "ws-chat-a".to_string(),
        token.clone(),
    );

    assert!(!cancel_ws_turn(&turn_id, "client-b", "ws-chat-a"));
    assert!(!token.is_cancelled());
    assert!(!cancel_ws_turn(&turn_id, "client-a", "ws-chat-b"));
    assert!(!token.is_cancelled());

    assert!(cancel_ws_turn(&turn_id, "client-a", "ws-chat-a"));
    assert!(token.is_cancelled());
    assert!(!cancel_ws_turn(&turn_id, "client-a", "ws-chat-a"));
}

#[tokio::test]
async fn websocket_turn_stop_does_not_cancel_another_client() {
    let first_turn = format!("turn-first-{}", uuid::Uuid::new_v4());
    let second_turn = format!("turn-second-{}", uuid::Uuid::new_v4());
    let first_token = crate::tools::subagent::CancellationToken::new();
    let second_token = crate::tools::subagent::CancellationToken::new();
    register_ws_turn(
        first_turn.clone(),
        "client-a".to_string(),
        "ws-chat".to_string(),
        first_token.clone(),
    );
    register_ws_turn(
        second_turn.clone(),
        "client-b".to_string(),
        "ws-chat".to_string(),
        second_token.clone(),
    );

    assert!(cancel_ws_turn(&first_turn, "client-a", "ws-chat"));
    assert!(first_token.is_cancelled());
    assert!(!second_token.is_cancelled());

    cancel_ws_turn(&second_turn, "client-b", "ws-chat");
}
