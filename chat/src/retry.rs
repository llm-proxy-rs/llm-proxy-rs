use aws_sdk_bedrockruntime::error::SdkError;
use std::future::Future;
use std::time::Duration;

/// Maximum number of exponential-backoff retries for transient Bedrock errors.
pub const MAX_RETRY_ATTEMPTS: u32 = 5;

const RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
const RETRY_MAX_DELAY: Duration = Duration::from_secs(30);

/// Tunable retry policy used by Bedrock request helpers.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::production()
    }
}

impl RetryPolicy {
    pub const fn production() -> Self {
        Self {
            max_attempts: MAX_RETRY_ATTEMPTS,
            base_delay: RETRY_BASE_DELAY,
            max_delay: RETRY_MAX_DELAY,
        }
    }

    #[cfg(test)]
    pub const fn fast_for_tests() -> Self {
        Self {
            max_attempts: MAX_RETRY_ATTEMPTS,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        }
    }
}

/// HTTP status codes that indicate a transient upstream failure worth retrying.
fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

/// Returns true when a Bedrock SDK error is likely transient and safe to retry.
pub fn is_retryable_sdk_error<E>(err: &SdkError<E>) -> bool {
    match err {
        SdkError::DispatchFailure(_) | SdkError::TimeoutError(_) => true,
        SdkError::ServiceError(service_err) => {
            is_retryable_http_status(service_err.raw().status().as_u16())
        }
        _ => false,
    }
}

/// Exponential backoff delay for the given zero-based attempt index.
pub fn retry_delay(attempt: u32) -> Duration {
    retry_delay_for_policy(attempt, &RetryPolicy::production())
}

pub fn retry_delay_for_policy(attempt: u32, policy: &RetryPolicy) -> Duration {
    let multiplier = 1u32.checked_shl(attempt).unwrap_or(u32::MAX);
    policy
        .base_delay
        .saturating_mul(multiplier)
        .min(policy.max_delay)
}

/// Runs `operation` with exponential backoff while errors remain retryable.
pub async fn with_exponential_retry<T, E, R, F, Fut>(
    policy: &RetryPolicy,
    mut is_retryable: R,
    mut operation: F,
) -> Result<T, E>
where
    R: FnMut(&E) -> bool,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut retry_attempt = 0u32;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if is_retryable(&err) && retry_attempt < policy.max_attempts => {
                tokio::time::sleep(retry_delay_for_policy(retry_attempt, policy)).await;
                retry_attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::{
        operation::converse_stream::ConverseStreamError,
        types::error::{InternalServerException, ThrottlingException, ValidationException},
    };
    use aws_smithy_runtime_api::http::{
        Response as SmithyResponse, StatusCode as SmithyStatusCode,
    };
    use aws_smithy_types::{body::SdkBody, error::ErrorMetadata};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn service_error(status: u16, err: ConverseStreamError) -> SdkError<ConverseStreamError> {
        let raw = SmithyResponse::new(
            SmithyStatusCode::try_from(status).unwrap(),
            SdkBody::from("error"),
        );
        SdkError::service_error(err, raw)
    }

    #[test]
    fn retryable_for_throttling() {
        let err = service_error(
            429,
            ConverseStreamError::ThrottlingException(
                ThrottlingException::builder()
                    .message("rate limited")
                    .meta(ErrorMetadata::builder().message("rate limited").build())
                    .build(),
            ),
        );
        assert!(is_retryable_sdk_error(&err));
    }

    #[test]
    fn retryable_for_internal_server_error() {
        let err = service_error(
            500,
            ConverseStreamError::InternalServerException(
                InternalServerException::builder()
                    .message("internal")
                    .meta(ErrorMetadata::builder().message("internal").build())
                    .build(),
            ),
        );
        assert!(is_retryable_sdk_error(&err));
    }

    #[test]
    fn not_retryable_for_validation_error() {
        let err = service_error(
            400,
            ConverseStreamError::ValidationException(
                ValidationException::builder()
                    .message("bad request")
                    .meta(ErrorMetadata::builder().message("bad request").build())
                    .build(),
            ),
        );
        assert!(!is_retryable_sdk_error(&err));
    }

    #[test]
    fn retry_delay_grows_exponentially_and_caps() {
        assert_eq!(retry_delay(0), Duration::from_millis(500));
        assert_eq!(retry_delay(1), Duration::from_secs(1));
        assert_eq!(retry_delay(2), Duration::from_secs(2));
        assert_eq!(retry_delay(10), RETRY_MAX_DELAY);
    }

    #[tokio::test(start_paused = true)]
    async fn with_exponential_retry_succeeds_after_transient_failures() {
        let policy = RetryPolicy::fast_for_tests();
        let calls = AtomicU32::new(0);

        let result: Result<&str, u16> = with_exponential_retry(
            &policy,
            |code: &u16| *code == 429,
            || async {
                let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if n <= 2 { Err(429) } else { Ok("ok") }
            },
        )
        .await;

        assert_eq!(result, Ok("ok"));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn with_exponential_retry_does_not_retry_non_retryable_errors() {
        let policy = RetryPolicy::fast_for_tests();
        let calls = AtomicU32::new(0);

        let result: Result<(), u16> = with_exponential_retry(
            &policy,
            |code: &u16| *code == 429,
            || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(400u16)
            },
        )
        .await;

        assert_eq!(result, Err(400));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn with_exponential_retry_stops_after_max_attempts() {
        let policy = RetryPolicy {
            max_attempts: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
        };
        let calls = AtomicU32::new(0);

        let result: Result<(), u16> = with_exponential_retry(
            &policy,
            |code: &u16| *code == 503,
            || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(503u16)
            },
        )
        .await;

        assert_eq!(result, Err(503));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
}
