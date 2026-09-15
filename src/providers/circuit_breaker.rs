use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Current state of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Failure threshold exceeded — requests are short-circuited.
    Open,
    /// Probing whether the downstream has recovered.
    HalfOpen,
}

/// Tracks consecutive failures and implements a simple circuit-breaker
/// with automatic half-open probing.
#[derive(Debug)]
pub struct CircuitBreakerInner {
    state: CircuitState,
    consecutive_failures: usize,
    last_failure: Option<Instant>,
    threshold: usize,
    reset_timeout: Duration,
}

impl CircuitBreakerInner {
    pub fn new(threshold: usize, reset_timeout: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            last_failure: None,
            threshold,
            reset_timeout,
        }
    }

    /// Returns `Ok(())` if the request is allowed, `Err(())` if the circuit is open.
    fn check(&mut self) -> Result<(), ()> {
        match self.state {
            CircuitState::Closed | CircuitState::HalfOpen => Ok(()),
            CircuitState::Open => {
                // Check if enough time has passed to transition to HalfOpen
                if let Some(last) = self.last_failure {
                    if last.elapsed() >= self.reset_timeout {
                        self.state = CircuitState::HalfOpen;
                        return Ok(());
                    }
                }
                Err(())
            }
        }
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.state = CircuitState::Closed;
        self.last_failure = None;
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_failure = Some(Instant::now());
        if self.consecutive_failures >= self.threshold {
            self.state = CircuitState::Open;
        }
    }

    fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.consecutive_failures = 0;
        self.last_failure = None;
    }
}

/// Thread-safe wrapper around `CircuitBreakerInner`.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    inner: Arc<Mutex<CircuitBreakerInner>>,
}

impl CircuitBreaker {
    pub fn new(threshold: usize, reset_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CircuitBreakerInner::new(
                threshold,
                reset_timeout,
            ))),
        }
    }

    /// Returns `Ok(())` if the request is allowed, `Err(msg)` if the circuit is open.
    pub fn check(&self) -> Result<(), String> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .check()
            .map_err(|_| "Provider circuit breaker is open — too many recent failures".to_string())
    }

    /// Record a successful API call.
    pub fn record_success(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.record_success();
    }

    /// Record a failed API call.
    pub fn record_failure(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.record_failure();
    }

    /// Manually reset the circuit breaker.
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.reset();
    }

    #[cfg(test)]
    fn state(&self) -> CircuitState {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).state
    }
}

/// HTTP status codes that are considered retryable (transient server-side failures).
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}

/// Compute the next backoff duration for a given attempt (1-indexed).
/// Uses: `min(base * 2^(attempt-1), max_delay)`.
pub fn backoff_duration(attempt: usize, base: Duration, max_delay: Duration) -> Duration {
    let mut delay = base;
    for _ in 1..attempt {
        delay = delay.saturating_mul(2);
        if delay >= max_delay {
            return max_delay;
        }
    }
    delay.min(max_delay)
}

/// Retry loop that respects a circuit breaker.
///
/// Wraps an async `operation` closure. On retryable HTTP status codes (429/502/503/504)
/// the call is retried with exponential backoff. Non-retryable errors are returned
/// immediately. The circuit breaker is updated on each attempt.
///
/// # Arguments
/// * `breaker` — Shared circuit breaker.
/// * `max_retries` — Maximum number of retry attempts (not counting the initial call).
/// * `base_delay` — Initial backoff delay (doubled each retry).
/// * `max_delay` — Upper bound for backoff.
/// * `provider_name` — Used in error messages.
/// * `operation` — Async closure that returns `Result<T, (u16, String)>` where the
///   tuple is `(http_status, error_body)`.
pub async fn retry_with_backoff<F, T, Fut>(
    breaker: &CircuitBreaker,
    max_retries: usize,
    base_delay: Duration,
    max_delay: Duration,
    provider_name: &str,
    operation: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, (u16, String)>>,
{
    // Check circuit breaker before the first attempt.
    if let Err(msg) = breaker.check() {
        return Err(anyhow::anyhow!(
            "Provider '{}' error (HTTP 503): {}",
            provider_name,
            msg
        ));
    }

    let mut last_err = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = backoff_duration(attempt, base_delay, max_delay);
            tokio::time::sleep(delay).await;
        }

        match operation().await {
            Ok(result) => {
                breaker.record_success();
                return Ok(result);
            }
            Err((status, error_text)) => {
                last_err = Some((status, error_text.clone()));

                if is_retryable_status(status) {
                    breaker.record_failure();
                    // If the circuit opened during this attempt, stop retrying.
                    if breaker.check().is_err() {
                        return Err(anyhow::anyhow!(
                            "Provider '{}' error (HTTP {}): Circuit breaker opened after {}/{} retries. Last error: {}",
                            provider_name,
                            status,
                            attempt,
                            max_retries,
                            error_text
                        ));
                    }
                } else {
                    // Non-retryable — return the error immediately.
                    return Err(anyhow::anyhow!(
                        "Provider '{}' error (HTTP {}): {}",
                        provider_name,
                        status,
                        error_text
                    ));
                }
            }
        }
    }

    // All retries exhausted.
    let (status, error_text) = last_err.unwrap_or((0, "Unknown error".into()));
    Err(anyhow::anyhow!(
        "Provider '{}' error (HTTP {}): All retries exhausted ({max_retries} attempts). Last error: {}",
        provider_name,
        status,
        error_text
    ))
}

#[cfg(test)]
#[path = "circuit_breaker_tests.rs"]
mod tests;

