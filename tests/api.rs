use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use futures_util::{SinkExt, StreamExt};
use randomizer::{build_router, error::ErrorResponse};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

#[tokio::test]
async fn rest_generates_json_from_valid_schema() {
    let response = build_router(4)
        .oneshot(json_request(json!({
            "type": "object",
            "properties": {
                "id": {"type": "int", "min": 5, "max": 5}
            }
        })))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, json!({"id": 5}));
}

#[tokio::test]
async fn rest_returns_structured_errors_for_invalid_input() {
    let invalid_schema = build_router(4)
        .oneshot(json_request(json!({
            "type": "boolean",
            "true_probability": 101
        })))
        .await
        .unwrap();
    assert_eq!(invalid_schema.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = serde_json::from_value(response_json(invalid_schema).await).unwrap();
    assert_eq!(body.code, "invalid_schema");

    let malformed = Request::builder()
        .method("POST")
        .uri("/generate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"))
        .unwrap();
    let malformed = build_router(4).oneshot(malformed).await.unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let body: ErrorResponse = serde_json::from_value(response_json(malformed).await).unwrap();
    assert_eq!(body.code, "invalid_request");
}

#[tokio::test]
async fn websocket_streams_values_and_reports_protocol_errors() {
    let (address, server) = spawn_server().await;
    let (mut socket, _) = connect_async(format!("ws://{address}/stream"))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            json!({
                "frequency": 100,
                "schema": {"type": "int", "min": 9, "max": 9}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(message.into_text().unwrap(), "9");
    socket.close(None).await.unwrap();

    let (mut socket, _) = connect_async(format!("ws://{address}/stream"))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            json!({
                "frequency": 99,
                "schema": {"type": "boolean", "true_probability": 50}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    let message = socket.next().await.unwrap().unwrap().into_text().unwrap();
    let error: ErrorResponse = serde_json::from_str(&message).unwrap();
    assert_eq!(error.code, "invalid_frequency");

    server.abort();
}

fn json_request(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/generate")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn spawn_server() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, build_router(4)).await.unwrap();
    });
    (address, server)
}
