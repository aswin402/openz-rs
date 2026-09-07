//! Shared HTTP client utilities with secure TLS and sensible default timeouts.

use std::time::Duration;

pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Build a standard HTTP client configured with rustls TLS and standard timeouts.
pub fn default_http_client() -> reqwest::Client {
    custom_http_client(DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT)
}

/// Build an HTTP client with custom connect and request timeouts.
pub fn custom_http_client(connect_timeout: Duration, request_timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .use_rustls_tls()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .unwrap_or_else(|err| {
            tracing::warn!(error = ?err, "failed to build custom reqwest client with rustls, falling back to default TLS client with preserved timeouts");
            reqwest::Client::builder()
                .connect_timeout(connect_timeout)
                .timeout(request_timeout)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
}

#[cfg(test)]
mod tests {
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
    }
}

