use axum::{Json, extract::rejection::JsonRejection};
use tracing::{debug, warn};

use crate::{
    error::ApiError,
    generation::{GenerationResult, generate_value},
    schema::RestGenerateRequest,
};

/// Compiles an incoming schema and returns one value with deterministic replay metadata.
///
/// Invalid schema bounds are surfaced as `400 Bad Request`.
pub async fn generate(
    payload: Result<Json<RestGenerateRequest>, JsonRejection>,
) -> Result<Json<GenerationResult>, ApiError> {
    let Json(request) = payload.map_err(|error| ApiError::invalid_request(error.body_text()))?;
    let (schema, options) = request.into_parts();
    debug!(schema = ?schema, generation = ?options, "received generation request");

    let result = generate_value(&schema, &options).map_err(|error| {
        warn!(error = %error, "generation failed");
        ApiError::from(error)
    })?;
    debug!(response = %result.value, metadata = ?result.metadata, "generated response");
    Ok(Json(result))
}
