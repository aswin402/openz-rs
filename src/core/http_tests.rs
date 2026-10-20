use super::*;

#[test]
fn test_default_http_client_builds() {
    let client = default_http_client();
    // Client builds successfully and can be cloned
    let _ = client.clone();
}

#[test]
fn test_custom_http_client_with_custom_timeouts() {
    let client = custom_http_client(Duration::from_secs(5), Duration::from_secs(15));
    let _ = client.clone();
    let timeout_client = http_client_with_timeout(Duration::from_secs(3));
    let _ = timeout_client.clone();
    let read_client = http_client_with_read_timeout(
        Duration::from_secs(5),
        Duration::from_secs(10),
        Duration::from_secs(20),
    );
    let _ = read_client.clone();
}
