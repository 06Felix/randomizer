use std::{io, net::SocketAddr};

use axum::{
    Router,
    routing::{any, get, post},
};
use thiserror::Error;
use tracing::info;

use crate::{
    Config,
    api::{generate, stream, validate_contract},
    mock::CompiledMockRegistry,
    mock::{health, list_requests, list_routes, mock_request, reset},
    project::{ProjectManifest, ProjectPaths},
    state::AppState,
};

pub fn build_router(max_concurrent_ws_streams: usize) -> Router {
    Router::new()
        .route("/generate", post(generate))
        .route("/validate", post(validate_contract))
        .route("/stream", get(stream))
        .with_state(AppState::new(max_concurrent_ws_streams))
}

pub fn build_project_router(
    max_concurrent_ws_streams: usize,
    registry: CompiledMockRegistry,
) -> Router {
    Router::new()
        .route("/generate", post(generate))
        .route("/validate", post(validate_contract))
        .route("/stream", get(stream))
        .route("/__randomizer/health", get(health))
        .route("/__randomizer/routes", get(list_routes))
        .route("/__randomizer/requests", get(list_requests))
        .route("/__randomizer/reset", post(reset))
        .route("/mock/{service}", any(mock_request))
        .route("/mock/{service}/{*path}", any(mock_request))
        .with_state(AppState::new(max_concurrent_ws_streams).with_mock_registry(registry))
}

pub async fn run_project(
    manifest: &ProjectManifest,
    paths: &ProjectPaths,
) -> Result<(), ServerError> {
    let registry = CompiledMockRegistry::compile(manifest, paths).map_err(ServerError::Mock)?;
    let address = SocketAddr::new(
        manifest
            .project
            .host
            .parse()
            .map_err(|source| ServerError::InvalidHost {
                host: manifest.project.host.clone(),
                source,
            })?,
        manifest.project.port,
    );
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|source| ServerError::Bind { address, source })?;
    info!(%address, routes = manifest.routes.len(), "project mock server listening");
    axum::serve(
        listener,
        build_project_router(AppState::DEFAULT_MAX_CONCURRENT_WS_STREAMS, registry),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(ServerError::Serve)
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
    #[error("invalid project host {host:?}: {source}")]
    InvalidHost {
        host: String,
        #[source]
        source: std::net::AddrParseError,
    },
    #[error("failed to compile project mocks: {0}")]
    Mock(#[source] crate::mock::MockCompileError),
    #[error("failed to bind server to {address}: {source}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("server failed: {0}")]
    Serve(#[source] io::Error),
}
