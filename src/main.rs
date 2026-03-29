use axum::{
    Router,
    routing::{get, post},
};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::api::{generate, stream};
use crate::state::AppState;

mod api;
mod compiler;
mod generator;
mod schema;
mod state;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let app_state = AppState::new(AppState::DEFAULT_MAX_CONCURRENT_WS_STREAMS);
    let app = Router::new()
        .route("/generate", post(generate))
        .route("/stream", get(stream))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:7263").await.unwrap();

    info!("server listening on 0.0.0.0:7263");

    axum::serve(listener, app).await.unwrap();
}
