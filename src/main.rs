use axum::{Router, routing::post};

use crate::api::get_random;

mod api;
mod compiler;
mod generator;
mod schema;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/random", post(get_random)); // 👈 use handler here

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("Server running on 3000");

    axum::serve(listener, app).await.unwrap();
}
