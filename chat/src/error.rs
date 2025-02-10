use axum::{http::StatusCode, response::IntoResponse};
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct StreamError(pub String);

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for StreamError {}

impl IntoResponse for StreamError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Stream error: {}", self.0),
        )
            .into_response()
    }
}
