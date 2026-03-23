use axum::{Json, response::IntoResponse};
use rand::rng;

use crate::{compiler::compile_schema, schema::Schema};

pub async fn get_random(Json(body): Json<Schema>) -> impl IntoResponse {
    let compiledg = compile_schema(&body);
    let mut rng = rng();
    Json(compiledg.generate(&mut rng))
}
