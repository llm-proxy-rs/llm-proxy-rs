use anyhow::Error as AnyhowError;
use aws_sdk_bedrockruntime::{
    error::SdkError,
    operation::{converse_stream::ConverseStreamError, count_tokens::CountTokensError},
};
use axum::{http::StatusCode, response::IntoResponse};

pub struct AppError(StatusCode, String);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (self.0, self.1).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<AnyhowError>,
{
    fn from(err: E) -> Self {
        let err: AnyhowError = err.into();
        let status = err
            .downcast_ref::<SdkError<ConverseStreamError>>()
            .and_then(|e| e.raw_response())
            .map(|r| r.status().as_u16())
            .or_else(|| {
                err.downcast_ref::<SdkError<CountTokensError>>()
                    .and_then(|e| e.raw_response())
                    .map(|r| r.status().as_u16())
            })
            .and_then(|code| StatusCode::from_u16(code).ok())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let message = err
            .downcast_ref::<SdkError<ConverseStreamError>>()
            .and_then(|e| e.as_service_error())
            .and_then(|se| se.meta().message())
            .or_else(|| {
                err.downcast_ref::<SdkError<CountTokensError>>()
                    .and_then(|e| e.as_service_error())
                    .and_then(|se| se.meta().message())
            })
            .map(String::from)
            .unwrap_or_else(|| err.to_string());
        Self(status, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::error::ValidationException;
    use aws_smithy_runtime_api::http::{
        Response as SmithyResponse, StatusCode as SmithyStatusCode,
    };
    use aws_smithy_types::body::SdkBody;
    use aws_smithy_types::error::ErrorMetadata;
    use http_body_util::BodyExt;

    fn make_converse_stream_error(status: u16) -> anyhow::Error {
        let raw = SmithyResponse::new(
            SmithyStatusCode::try_from(status).unwrap(),
            SdkBody::from("error"),
        );
        let err = ConverseStreamError::unhandled("test error");
        let sdk_err: SdkError<ConverseStreamError> = SdkError::service_error(err, raw);
        sdk_err.into()
    }

    fn make_count_tokens_error(status: u16) -> anyhow::Error {
        let raw = SmithyResponse::new(
            SmithyStatusCode::try_from(status).unwrap(),
            SdkBody::from("error"),
        );
        let err = CountTokensError::unhandled("test error");
        let sdk_err: SdkError<CountTokensError> = SdkError::service_error(err, raw);
        sdk_err.into()
    }

    #[test]
    fn converse_stream_error_preserves_429() {
        let app_error = AppError::from(make_converse_stream_error(429));
        assert_eq!(app_error.0, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn converse_stream_error_preserves_400() {
        let app_error = AppError::from(make_converse_stream_error(400));
        assert_eq!(app_error.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn count_tokens_error_preserves_429() {
        let app_error = AppError::from(make_count_tokens_error(429));
        assert_eq!(app_error.0, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn generic_error_defaults_to_500() {
        let app_error = AppError::from(anyhow::anyhow!("something broke"));
        assert_eq!(app_error.0, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn validation_exception_extracts_message() {
        let expected = "The model returned the following errors: invalid beta flag";
        let raw = SmithyResponse::new(
            SmithyStatusCode::try_from(400).unwrap(),
            SdkBody::from(format!(r#"{{"message":"{expected}"}}"#)),
        );
        let err = ConverseStreamError::ValidationException(
            ValidationException::builder()
                .message(expected)
                .meta(ErrorMetadata::builder().message(expected).build())
                .build(),
        );
        let sdk_err: SdkError<ConverseStreamError> = SdkError::service_error(err, raw);
        let app_error = AppError::from(anyhow::Error::from(sdk_err));

        assert_eq!(app_error.0, StatusCode::BAD_REQUEST);

        let response = app_error.into_response();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(String::from_utf8(body.to_vec()).unwrap(), expected);
    }
}
