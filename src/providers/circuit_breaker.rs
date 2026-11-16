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
                let recovered = self
                    .last_failure
                    .is_some_and(|last| last.elapsed() >= self.reset_timeout);
                match recovered {
                    true => {
                        self.state = CircuitState::HalfOpen;
                        Ok(())
                    }
                    false => Err(()),
                }
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
        match self.consecutive_failures >= self.threshold {
            true => self.state = CircuitState::Open,
            false => {}
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
        match delay >= max_delay {
            true => return max_delay,
            false => {}
        }
    }
    delay.min(max_delay)
}

/// Equal-jitter exponential backoff (`[cap / 2, cap]`) to prevent synchronized retry storms.
pub fn backoff_duration_with_jitter(
    attempt: usize,
    base: Duration,
    max_delay: Duration,
) -> Duration {
    let cap_ms = backoff_duration(attempt, base, max_delay).as_millis() as u64;
    match cap_ms <= 1 {
        true => Duration::from_millis(cap_ms),
        false => {
            let half = cap_ms / 2;
            let span = (cap_ms - half).max(1);
            let entropy = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64)
                .unwrap_or(0);
            Duration::from_millis(half + (entropy % span))
        }
    }
}

/// Retry loop that respects a circuit breaker and optional `Retry-After` hints.
pub async fn retry_with_backoff_hint<F, T, Fut>(
    breaker: &CircuitBreaker,
    max_retries: usize,
    base_delay: Duration,
    max_delay: Duration,
    provider_name: &str,
    operation: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, (u16, String, Option<Duration>)>>,
{
    match breaker.check() {
        Ok(()) => {}
        Err(msg) => {
            return Err(anyhow::anyhow!(
                "Provider '{}' error (HTTP 503): {}",
                provider_name,
                msg
            ));
        }
    }

    let mut last_err: Option<(u16, String)> = None;
    let mut pending_server_delay: Option<Duration> = None;

    for attempt in 0..=max_retries {
        match attempt > 0 {
            true => {
                let delay = pending_server_delay
                    .take()
                    .map(|d| d.min(max_delay))
                    .unwrap_or_else(|| backoff_duration_with_jitter(attempt, base_delay, max_delay));
                tokio::time::sleep(delay).await;
            }
            false => {}
        }

        match operation().await {
            Ok(result) => {
                breaker.record_success();
                return Ok(result);
            }
            Err((status, error_text, retry_after)) => {
                last_err = Some((status, error_text.clone()));
                pending_server_delay = retry_after;

                match is_retryable_status(status) {
                    true => {
                        breaker.record_failure();
                        match breaker.check().is_err() {
                            true => {
                                return Err(anyhow::anyhow!(
                                    "Provider '{}' error (HTTP {}): Circuit breaker opened after {}/{} retries. Last error: {}",
                                    provider_name,
                                    status,
                                    attempt,
                                    max_retries,
                                    error_text
                                ));
                            }
                            false => {}
                        }
                    }
                    false => {
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
    }

    let (status, error_text) = last_err.unwrap_or((0, "Unknown error".into()));
    Err(anyhow::anyhow!(
        "Provider '{}' error (HTTP {}): All retries exhausted ({max_retries} attempts). Last error: {}",
        provider_name,
        status,
        error_text
    ))
}

/// Retry loop that respects a circuit breaker.
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
    retry_with_backoff_hint(
        breaker,
        max_retries,
        base_delay,
        max_delay,
        provider_name,
        || async {
            operation()
                .await
                .map_err(|(status, text)| (status, text, None))
        },
    )
    .await
}

#[cfg(test)]
#[path = "circuit_breaker_tests.rs"]
mod tests;
