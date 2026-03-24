use axum::{Json, http::StatusCode, response::IntoResponse};
use rand::rng;

use crate::{compiler::compile_schema, schema::Schema};

pub async fn get_random(Json(body): Json<Schema>) -> impl IntoResponse {
    match compile_schema(&body) {
        Ok(generator) => {
            let mut rng = rng();
            Json(generator.generate(&mut rng)).into_response()
        }
        Err(error) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    }
}
