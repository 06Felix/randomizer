use axum::{Json, extract::rejection::JsonRejection};
use tracing::{debug, warn};

use crate::{
    error::ApiError,
    generation::{GenerationResult, generate_value},
    schema::{RestGenerateRequest, ValidateContractRequest},
    standard::{ValidationReport, generate_standard_value, validate_standard_value},
};

/// Compiles an incoming schema and returns one value with deterministic replay metadata.
///
/// Invalid schema bounds are surfaced as `400 Bad Request`.
pub async fn generate(
    payload: Result<Json<RestGenerateRequest>, JsonRejection>,
) -> Result<Json<GenerationResult>, ApiError> {
    let Json(request) = payload.map_err(|error| ApiError::invalid_request(error.body_text()))?;
    let result = match request {
        RestGenerateRequest::Standard(request) => {
            let options = request.generation_options();
            debug!(contract = %request.contract.name, mode = ?request.mode, generation = ?options, "received standard contract generation request");
            generate_standard_value(request.contract, request.mode, &options)
        }
        custom => {
            let (schema, options) = custom
                .into_custom_parts()
                .ok_or_else(|| ApiError::invalid_request("invalid custom generation request"))?;
            debug!(schema = ?schema, generation = ?options, "received custom generation request");
            generate_value(&schema, &options)
        }
    }
    .map_err(|error| {
        warn!(error = %error, "generation failed");
        ApiError::from(error)
    })?;
    debug!(response = %result.value, metadata = ?result.metadata, "generated response");
    Ok(Json(result))
}

/// Imports a Draft 2020-12 contract and validates a supplied JSON value.
pub async fn validate_contract(
    payload: Result<Json<ValidateContractRequest>, JsonRejection>,
) -> Result<Json<ValidationReport>, ApiError> {
    let Json(request) = payload.map_err(|error| ApiError::invalid_request(error.body_text()))?;
    let report =
        validate_standard_value(request.contract, &request.value).map_err(ApiError::from)?;
    Ok(Json(report))
}
