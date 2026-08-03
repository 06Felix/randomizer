use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorResponse,
}

impl ApiError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            body: ErrorResponse::new(code, message),
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub fn invalid_schema(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_schema", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("minimum {min} is greater than maximum {max}")]
    InvalidRange { min: String, max: String },
    #[error("precision {precision} exceeds the maximum supported precision {maximum}")]
    InvalidPrecision { precision: u8, maximum: u8 },
    #[error("true_probability must be between 0 and 100, got {0}")]
    InvalidProbability(i32),
    #[error("provide either length or both min_length and max_length for {subject}")]
    MissingLength { subject: &'static str },
    #[error("min_length is greater than max_length for {subject}")]
    InvalidLengthRange { subject: &'static str },
    #[error("{subject} length cannot exceed {maximum}")]
    LengthTooLarge {
        subject: &'static str,
        maximum: usize,
    },
    #[error("custom strings require custom_charset")]
    MissingCustomCharset,
    #[error("custom_charset cannot be empty")]
    EmptyCustomCharset,
    #[error("enum values cannot be empty")]
    EmptyEnum,
    #[error("enum values must contain only string, number, or boolean values")]
    InvalidEnumValue,
    #[error("{path}: {source}")]
    AtPath {
        path: String,
        #[source]
        source: Box<CompileError>,
    },
}

impl CompileError {
    pub fn at_path(self, path: impl Into<String>) -> Self {
        Self::AtPath {
            path: path.into(),
            source: Box::new(self),
        }
    }
}
