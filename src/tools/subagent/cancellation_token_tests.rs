use super::*;

use crate::tools::subagent::cancel_test_guard;

#[tokio::test]
async fn test_cancellation_token_observes_cli_cancel_signal() {
    let _guard = cancel_test_guard().await;
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());

    crate::shutdown::trigger_cli_cancel();

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        token.wait_for_cancellation(),
    )
    .await
    .expect("token should observe CLI cancel signal");
    assert!(token.is_cancelled());
}
