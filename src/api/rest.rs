use axum::{Json, http::StatusCode, response::IntoResponse};
use rand::rng;
use serde::Serialize;
use tracing::{debug, warn};

use crate::{compiler::compile_schema, schema::Schema};

#[derive(Serialize)]
struct RestErrorBody {
    error: String,
}

/// Compiles an incoming schema and returns one random JSON value for it.
///
/// Invalid schema bounds are surfaced as `400 Bad Request`.
pub async fn generate(Json(body): Json<Schema>) -> impl IntoResponse {
    debug!(schema = ?body, "received random generation request");

    match compile_schema(&body) {
        Ok(generator) => {
            let mut rng = rng();
            let value = generator.generate(&mut rng);
            debug!(response = %value, "generated random response");
            Json(value).into_response()
        }
        Err(error) => {
            warn!(error = %error, "schema compilation failed");
            (
                StatusCode::BAD_REQUEST,
                Json(RestErrorBody {
                    error: error.to_string(),
                }),
            )
                .into_response()
        }
    }
}
