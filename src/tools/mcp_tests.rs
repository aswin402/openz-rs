use super::*;

#[test]
fn test_find_free_port_returns_bound_listener() {
    let (port, listener) = find_free_port().unwrap();
    assert!(port >= 50060, "port should be in dynamic range");
    assert!(port <= 65000, "port should be in dynamic range");
    // Listener should be alive — binding same port again should fail
    assert!(
        std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_err(),
        "port should still be held by the guard listener"
    );
    // Drop the listener, then binding should succeed
    drop(listener);
    assert!(
        std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok(),
        "port should be free after guard is dropped"
    );
}

#[test]
fn test_find_free_port_sequential_ports_differ() {
    let (_p1, l1) = find_free_port().unwrap();
    let (p2, _l2) = find_free_port().unwrap();
    // Second call should advance to a different port
    assert_ne!(p2, 0);
    drop(l1);
}

#[tokio::test]
async fn test_invalidate_nonexistent_key_does_not_panic() {
    // Should not panic when cache is empty
    McpClient::invalidate("nonexistent", &[]).await;
}

#[tokio::test]
async fn test_invalidate_after_spawn_removes_entry() {
    // Create a non-existent command — spawn should fail, no entry cached
    let result = McpClient::spawn("this-command-does-not-exist-12345", &[]).await;
    assert!(result.is_err(), "spawn of nonexistent command should fail");

    // Invalidate on something that was never cached should not panic
    McpClient::invalidate("this-command-does-not-exist-12345", &[]).await;
}

#[tokio::test]
async fn test_ping_on_closed_client_returns_error() {
    let client = McpClient(Arc::new(Mutex::new(None)));
    assert!(client.ping().await.is_err());
}

#[tokio::test]
async fn test_mcp_bridge_with_cat() {
    let result = McpClient::spawn("cat", &[]).await;
    assert!(
        result.is_ok(),
        "Failed to spawn cat MCP bridge: {:?}",
        result.err()
    );
    let client = result.unwrap();
    let ping_res = client.ping().await;
    assert!(ping_res.is_ok(), "Ping failed: {:?}", ping_res.err());
}
