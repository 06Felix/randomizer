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

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
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

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error(transparent)]
    InvalidSchema(#[from] CompileError),
    #[error("unsupported generator_version {provided:?}; this server supports {supported:?}")]
    UnsupportedGeneratorVersion {
        provided: String,
        supported: &'static str,
    },
    #[error(
        "contract_hash does not match the canonical schema: expected {expected}, got {provided}"
    )]
    ContractHashMismatch { expected: String, provided: String },
    #[error("failed to canonicalize schema: {0}")]
    Canonicalization(#[source] serde_json::Error),
    #[error(transparent)]
    StandardContract(#[from] StandardContractError),
}

#[derive(Debug, Error)]
pub enum StandardContractError {
    #[error("contract {field} must not be empty")]
    EmptyMetadata { field: &'static str },
    #[error("invalid JSON Schema Draft 2020-12 contract: {0}")]
    InvalidSchema(String),
    #[error("unsupported JSON Schema dialect {dialect:?}; expected Draft 2020-12")]
    UnsupportedDialect { dialect: String },
    #[error("external reference {reference:?} is not supported; use a local # JSON Pointer")]
    ExternalReference { reference: String },
    #[error("unresolved local reference {reference:?}")]
    UnresolvedReference { reference: String },
    #[error("cyclic reference {reference:?} cannot be generated")]
    CyclicReference { reference: String },
    #[error("unsupported generation at {schema_path}: {reason}")]
    UnsupportedGeneration { schema_path: String, reason: String },
    #[error("generated value failed contract validation: {0}")]
    GeneratedValueInvalid(String),
    #[error("invalid mode could not produce a contract violation")]
    UnableToProduceInvalidValue,
}

impl From<GenerationError> for ApiError {
    fn from(error: GenerationError) -> Self {
        let message = error.to_string();
        match error {
            GenerationError::InvalidSchema(_) => Self::invalid_schema(message),
            GenerationError::UnsupportedGeneratorVersion { .. }
            | GenerationError::ContractHashMismatch { .. } => Self::invalid_request(message),
            GenerationError::Canonicalization(_) => Self::internal(message),
            GenerationError::StandardContract(StandardContractError::GeneratedValueInvalid(_))
            | GenerationError::StandardContract(
                StandardContractError::UnableToProduceInvalidValue,
            ) => Self::internal(message),
            GenerationError::StandardContract(_) => Self::invalid_request(message),
        }
    }
}

impl CompileError {
    pub fn at_path(self, path: impl Into<String>) -> Self {
        Self::AtPath {
            path: path.into(),
            source: Box::new(self),
        }
    }
}
