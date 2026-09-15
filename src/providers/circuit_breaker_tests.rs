use super::*;

#[test]
fn test_backoff_duration() {
    let base = Duration::from_secs(1);
    let max = Duration::from_secs(10);

    assert_eq!(backoff_duration(1, base, max), Duration::from_secs(1));
    assert_eq!(backoff_duration(2, base, max), Duration::from_secs(2));
    assert_eq!(backoff_duration(3, base, max), Duration::from_secs(4));
    assert_eq!(backoff_duration(4, base, max), Duration::from_secs(8));
    assert_eq!(backoff_duration(5, base, max), Duration::from_secs(10)); // capped
    assert_eq!(backoff_duration(100, base, max), Duration::from_secs(10)); // capped
}

#[test]
fn test_is_retryable_status() {
    assert!(is_retryable_status(429));
    assert!(is_retryable_status(502));
    assert!(is_retryable_status(503));
    assert!(is_retryable_status(504));
    assert!(!is_retryable_status(200));
    assert!(!is_retryable_status(400));
    assert!(!is_retryable_status(401));
    assert!(!is_retryable_status(403));
    assert!(!is_retryable_status(500));
    assert!(!is_retryable_status(501));
}

#[test]
fn test_circuit_breaker_initial_state() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(30));
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.check().is_ok());
}

#[test]
fn test_circuit_breaker_opens_after_threshold() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(30));
    assert!(cb.check().is_ok());
    cb.record_failure();
    assert!(cb.check().is_ok());
    cb.record_failure();
    assert!(cb.check().is_ok());
    cb.record_failure(); // 3rd failure — threshold reached
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(cb.check().is_err());
}

#[test]
fn test_circuit_breaker_resets_on_success() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(30));
    cb.record_failure();
    cb.record_failure();
    cb.record_success(); // resets
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.check().is_ok());
}

#[test]
fn test_circuit_breaker_manual_reset() {
    let cb = CircuitBreaker::new(2, Duration::from_secs(30));
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.check().is_ok());
}
