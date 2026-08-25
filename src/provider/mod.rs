//! Language-neutral response-contract provider protocol and built-in importers.
//!
//! Providers are intentionally process-isolated: a language-specific adapter can
//! inspect its own build graph and types, then exchange a versioned JSON document
//! with Randomizer. The core only accepts Draft 2020-12 JSON Schema, keeping the
//! runtime independent of any application language.

mod import;
mod protocol;
mod runner;
mod schema_walk;

pub use import::{
    import_json_schema_bytes, import_json_schema_file, import_openapi_response_bytes,
    import_openapi_response_file, import_serialized_example_bytes, import_serialized_example_file,
};
pub use protocol::{
    DiagnosticSeverity, EVIDENCE_KIND_CONST, EVIDENCE_KIND_CONSTRAINT, EVIDENCE_KIND_ENUM,
    EVIDENCE_KIND_FORMAT, EVIDENCE_KIND_NULLABLE, EVIDENCE_KIND_PROPERTY_NAME,
    EVIDENCE_KIND_REQUIREDNESS, EVIDENCE_KIND_RESPONSE_WRAPPER, EVIDENCE_KIND_ROOT_SYMBOL,
    EVIDENCE_KIND_TYPE, EndpointSelector, FieldEvidence, ProviderDiagnostic, ProviderIdentity,
    ProviderRequest, ProviderResponse, SourceFingerprint, fingerprint_bytes,
    validate_provider_response,
};
pub use runner::{ProviderCommand, run_provider};

use std::{io, path::PathBuf, process::ExitStatus, time::Duration};

use thiserror::Error;

pub const PROVIDER_PROTOCOL_VERSION: &str = "1";
pub const JSON_SCHEMA_DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";
pub const SHA256_ALGORITHM: &str = "sha256";

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("failed to read provider source {path}: {source}")]
    ReadSource {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse provider source {path}: {message}")]
    ParseSource { path: String, message: String },
    #[error("unsupported OpenAPI document version {version:?}; expected OpenAPI 3.1.x")]
    UnsupportedOpenApiVersion { version: String },
    #[error(
        "unsupported OpenAPI jsonSchemaDialect {dialect:?}; expected Draft 2020-12 or the OpenAPI 3.1 base dialect"
    )]
    UnsupportedOpenApiDialect { dialect: String },
    #[error("OpenAPI path {path:?} was not found")]
    OpenApiPathNotFound { path: String },
    #[error("OpenAPI operation {method} {path} was not found")]
    OpenApiOperationNotFound { method: String, path: String },
    #[error("OpenAPI response {status} for {method} {path} was not found")]
    OpenApiResponseNotFound {
        method: String,
        path: String,
        status: u16,
    },
    #[error("OpenAPI response {status} for {method} {path} has no response content")]
    OpenApiResponseContentMissing {
        method: String,
        path: String,
        status: u16,
    },
    #[error("OpenAPI response content type {media_type:?} was not found; available: {available}")]
    OpenApiMediaTypeNotFound {
        media_type: String,
        available: String,
    },
    #[error(
        "OpenAPI response has multiple schema-bearing media types ({available}); specify media_type"
    )]
    AmbiguousOpenApiMediaType { available: String },
    #[error("OpenAPI response media type {media_type:?} has no schema")]
    OpenApiResponseSchemaMissing { media_type: String },
    #[error("external reference {reference:?} is not supported; use a local # JSON Pointer")]
    ExternalReference { reference: String },
    #[error("local reference {reference:?} is not a JSON Pointer")]
    InvalidLocalReference { reference: String },
    #[error("local reference {reference:?} could not be resolved")]
    UnresolvedReference { reference: String },
    #[error("cyclic local reference {reference:?} cannot be imported")]
    CyclicReference { reference: String },
    #[error("invalid provider endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("invalid provider response: {0}")]
    InvalidResponse(String),
    #[error("invalid Draft 2020-12 response schema: {0}")]
    InvalidSchema(String),
    #[error("failed to encode provider request: {0}")]
    EncodeRequest(#[source] serde_json::Error),
    #[error("failed to start provider command {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to write request to provider command {program}: {source}")]
    WriteRequest {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("provider command {program} timed out after {timeout:?}")]
    Timeout { program: String, timeout: Duration },
    #[error("failed while waiting for provider command {program}: {source}")]
    Wait {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("failed while reading provider command {program} {stream}: {source}")]
    ReadOutput {
        program: String,
        stream: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("provider command {program} exited with {status}: {stderr}")]
    Exit {
        program: String,
        status: ExitStatus,
        stderr: String,
    },
    #[error("provider command {program} returned {size} bytes; limit is {maximum} bytes")]
    OutputTooLarge {
        program: String,
        size: usize,
        maximum: usize,
    },
    #[error("provider command {program} returned invalid JSON: {source}")]
    DecodeResponse {
        program: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("provider command {program} returned endpoint {actual:?}; expected {expected:?}")]
    EndpointMismatch {
        program: String,
        expected: Box<EndpointSelector>,
        actual: Box<EndpointSelector>,
    },
}

pub type Result<T> = std::result::Result<T, ProviderError>;
