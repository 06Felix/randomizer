use axum::{Router, routing::{get, post}};

use crate::api::{get_random, ws_handler};

mod api;
mod compiler;
mod generator;
mod schema;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/random", post(get_random))
        .route("/ws", get(ws_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:7878").await.unwrap();

    println!("Server running on 7878");

    axum::serve(listener, app).await.unwrap();
}
