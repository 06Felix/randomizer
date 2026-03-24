use axum::{
    Router,
    routing::{get, post},
};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::api::{get_random, ws_handler};

mod api;
mod compiler;
mod generator;
mod schema;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
    let app = Router::new()
        .route("/random", post(get_random))
        .route("/ws", get(ws_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:7878").await.unwrap();

    info!("server listening on 0.0.0.0:7878");

    axum::serve(listener, app).await.unwrap();
}
