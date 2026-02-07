//! Retry policy and error classification for console-provider.
//!
//! This module provides pure types and functions for deciding **whether** and
//! **when** to retry a failed request.  It intentionally contains no async code,
//! no HTTP client, and no I/O — the actual retry loop lives in the caller.

use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error classification
// ---------------------------------------------------------------------------

/// Classification of an API error for retry decisions.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorClass {
    /// Transient error, safe to retry.  Optional delay hint from the server.
    Retryable { delay_hint: Option<Duration> },
    /// Rate limited.  Retry after the specified duration if available.
    RateLimit { retry_after: Option<Duration> },
    /// Fatal error, do not retry.
    Fatal,
    /// Authentication/authorization error — fix credentials, don't retry.
    AuthError,
}

// ---------------------------------------------------------------------------
// Retry policy
// ---------------------------------------------------------------------------

/// Configurable retry policy with exponential backoff and jitter.
///
/// The delay for attempt *n* (0-indexed) is:
///
/// ```text
/// delay = min(base_delay_ms * backoff_factor ^ n, max_delay_ms)
/// ```
///
/// A deterministic jitter of +/-25% is applied using the golden-ratio hash of
/// the attempt number so that no `rand` crate is needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay for the first retry (in milliseconds for serde).
    pub base_delay_ms: u64,
    /// Maximum delay cap (in milliseconds for serde).
    pub max_delay_ms: u64,
    /// Backoff multiplier applied per attempt.
    pub backoff_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1_000,
            max_delay_ms: 30_000,
            backoff_factor: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Classify an HTTP status code into an [`ErrorClass`].
    pub fn classify_http_status(status: u16) -> ErrorClass {
        match status {
            401 | 403 => ErrorClass::AuthError,
            429 => ErrorClass::RateLimit { retry_after: None },
            408 | 502 | 503 | 504 => ErrorClass::Retryable { delay_hint: None },
            400 | 404 | 405 | 422 => ErrorClass::Fatal,
            500 => ErrorClass::Retryable {
                delay_hint: Some(Duration::from_secs(2)),
            },
            _ if status >= 500 => ErrorClass::Retryable { delay_hint: None },
            _ => ErrorClass::Fatal,
        }
    }

    /// Classify an HTTP status with an optional `Retry-After` header value
    /// (interpreted as whole seconds).
    ///
    /// When the header is present the corresponding duration is propagated into
    /// the [`ErrorClass::RateLimit::retry_after`] or
    /// [`ErrorClass::Retryable::delay_hint`] field.
    pub fn classify_with_retry_after(status: u16, retry_after_secs: Option<u64>) -> ErrorClass {
        let mut class = Self::classify_http_status(status);
        if let Some(secs) = retry_after_secs {
            match &mut class {
                ErrorClass::RateLimit { retry_after } => {
                    *retry_after = Some(Duration::from_secs(secs));
                }
                ErrorClass::Retryable { delay_hint } => {
                    *delay_hint = Some(Duration::from_secs(secs));
                }
                _ => {}
            }
        }
        class
    }

    /// Calculate the delay for a given retry attempt (0-indexed).
    ///
    /// Uses exponential backoff with deterministic jitter (+/-25%).  Returns
    /// `None` if `attempt >= max_retries`.
    pub fn next_delay(&self, attempt: u32) -> Option<Duration> {
        if attempt >= self.max_retries {
            return None;
        }

        let base = self.base_delay_ms as f64;
        let delay = base * self.backoff_factor.powi(attempt as i32);
        let capped = delay.min(self.max_delay_ms as f64);

        // Deterministic jitter: use the golden ratio (φ − 1 ≈ 0.618…) to
        // produce a well-distributed fractional part per attempt, then scale
        // into the [0.75, 1.25] range (i.e. ±25%).
        let jitter_factor = 0.75 + 0.5 * ((attempt as f64 * 0.618_033_988) % 1.0);
        let final_ms = (capped * jitter_factor) as u64;

        Some(Duration::from_millis(final_ms.max(1)))
    }

    /// Whether a given [`ErrorClass`] should be retried.
    pub fn should_retry(class: &ErrorClass) -> bool {
        matches!(class, ErrorClass::Retryable { .. } | ErrorClass::RateLimit { .. })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, 3);
        assert_eq!(p.base_delay_ms, 1_000);
        assert_eq!(p.max_delay_ms, 30_000);
        assert!((p.backoff_factor - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_classify_auth_errors() {
        assert_eq!(
            RetryPolicy::classify_http_status(401),
            ErrorClass::AuthError
        );
        assert_eq!(
            RetryPolicy::classify_http_status(403),
            ErrorClass::AuthError
        );
    }

    #[test]
    fn test_classify_rate_limit() {
        assert_eq!(
            RetryPolicy::classify_http_status(429),
            ErrorClass::RateLimit { retry_after: None }
        );
    }

    #[test]
    fn test_classify_retryable() {
        for status in [408, 500, 502, 503, 504] {
            let class = RetryPolicy::classify_http_status(status);
            assert!(
                matches!(class, ErrorClass::Retryable { .. }),
                "expected Retryable for status {status}, got {class:?}"
            );
        }

        // 500 has a specific delay_hint
        assert_eq!(
            RetryPolicy::classify_http_status(500),
            ErrorClass::Retryable {
                delay_hint: Some(Duration::from_secs(2)),
            }
        );

        // Other 5xx codes should also be retryable
        assert_eq!(
            RetryPolicy::classify_http_status(507),
            ErrorClass::Retryable { delay_hint: None }
        );
    }

    #[test]
    fn test_classify_fatal() {
        for status in [400, 404, 405, 422] {
            assert_eq!(
                RetryPolicy::classify_http_status(status),
                ErrorClass::Fatal,
                "expected Fatal for status {status}"
            );
        }
    }

    #[test]
    fn test_classify_with_retry_after() {
        // 429 with Retry-After propagates the duration.
        let class = RetryPolicy::classify_with_retry_after(429, Some(60));
        assert_eq!(
            class,
            ErrorClass::RateLimit {
                retry_after: Some(Duration::from_secs(60)),
            }
        );

        // 503 with Retry-After propagates into delay_hint.
        let class = RetryPolicy::classify_with_retry_after(503, Some(5));
        assert_eq!(
            class,
            ErrorClass::Retryable {
                delay_hint: Some(Duration::from_secs(5)),
            }
        );

        // Fatal status ignores Retry-After.
        let class = RetryPolicy::classify_with_retry_after(400, Some(10));
        assert_eq!(class, ErrorClass::Fatal);

        // No Retry-After header leaves the default.
        let class = RetryPolicy::classify_with_retry_after(429, None);
        assert_eq!(
            class,
            ErrorClass::RateLimit { retry_after: None }
        );
    }

    #[test]
    fn test_next_delay_basic() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay_ms: 1_000,
            max_delay_ms: 60_000,
            backoff_factor: 2.0,
        };

        // Delays should generally increase with each attempt.
        let d0 = policy.next_delay(0).unwrap();
        let d1 = policy.next_delay(1).unwrap();
        let d2 = policy.next_delay(2).unwrap();

        // Because of jitter, we check that the *un-jittered* progression
        // (base, base*2, base*4) roughly holds — each delay should be in
        // a reasonable range.
        assert!(d0.as_millis() >= 750 && d0.as_millis() <= 1_250, "d0={d0:?}");
        assert!(d1.as_millis() >= 1_500 && d1.as_millis() <= 2_500, "d1={d1:?}");
        assert!(d2.as_millis() >= 3_000 && d2.as_millis() <= 5_000, "d2={d2:?}");
    }

    #[test]
    fn test_next_delay_capped() {
        let policy = RetryPolicy {
            max_retries: 10,
            base_delay_ms: 10_000,
            max_delay_ms: 15_000,
            backoff_factor: 4.0,
        };

        // Even at high attempts the delay must not exceed max_delay * 1.25
        // (the upper jitter bound).
        for attempt in 0..10 {
            let d = policy.next_delay(attempt).unwrap();
            assert!(
                d.as_millis() <= (15_000.0 * 1.25) as u128 + 1,
                "attempt {attempt}: delay {d:?} exceeds cap"
            );
        }
    }

    #[test]
    fn test_next_delay_exhausted() {
        let policy = RetryPolicy::default(); // max_retries = 3
        assert!(policy.next_delay(3).is_none());
        assert!(policy.next_delay(4).is_none());
        assert!(policy.next_delay(100).is_none());
    }

    #[test]
    fn test_should_retry() {
        assert!(RetryPolicy::should_retry(&ErrorClass::Retryable {
            delay_hint: None
        }));
        assert!(RetryPolicy::should_retry(&ErrorClass::RateLimit {
            retry_after: Some(Duration::from_secs(1))
        }));
        assert!(!RetryPolicy::should_retry(&ErrorClass::Fatal));
        assert!(!RetryPolicy::should_retry(&ErrorClass::AuthError));
    }

    #[test]
    fn test_policy_serialization() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay_ms: 500,
            max_delay_ms: 20_000,
            backoff_factor: 1.5,
        };

        let json = serde_json::to_string(&policy).expect("serialize");
        let roundtripped: RetryPolicy = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(roundtripped.max_retries, policy.max_retries);
        assert_eq!(roundtripped.base_delay_ms, policy.base_delay_ms);
        assert_eq!(roundtripped.max_delay_ms, policy.max_delay_ms);
        assert!((roundtripped.backoff_factor - policy.backoff_factor).abs() < f64::EPSILON);
    }
}
