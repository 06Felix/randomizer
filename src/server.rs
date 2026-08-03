use std::{io, net::SocketAddr};

use axum::{
    Router,
    routing::{get, post},
};
use thiserror::Error;
use tracing::info;

use crate::{
    Config,
    api::{generate, stream},
    state::AppState,
};

pub fn build_router(max_concurrent_ws_streams: usize) -> Router {
    Router::new()
        .route("/generate", post(generate))
        .route("/stream", get(stream))
        .with_state(AppState::new(max_concurrent_ws_streams))
}

pub async fn run(config: Config) -> Result<(), ServerError> {
    let address = SocketAddr::new(config.host, config.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|source| ServerError::Bind { address, source })?;

    info!(%address, "server listening");
    axum::serve(listener, build_router(config.max_concurrent_ws_streams))
        .await
        .map_err(ServerError::Serve)
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to bind server to {address}: {source}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("server failed: {0}")]
    Serve(#[source] io::Error),
}
