use axum::{Json, extract::rejection::JsonRejection};
use rand::rng;
use tracing::{debug, warn};

use crate::{error::ApiError, generation::generate_value, schema::Schema};

/// Compiles an incoming schema and returns one random JSON value for it.
///
/// Invalid schema bounds are surfaced as `400 Bad Request`.
pub async fn generate(
    payload: Result<Json<Schema>, JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Json(body) = payload.map_err(|error| ApiError::invalid_request(error.body_text()))?;
    debug!(schema = ?body, "received random generation request");

    let mut rng = rng();
    let value = generate_value(&body, &mut rng).map_err(|error| {
        warn!(error = %error, "schema compilation failed");
        ApiError::invalid_schema(error.to_string())
    })?;
    debug!(response = %value, "generated random response");
    Ok(Json(value))
}
