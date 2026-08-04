use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use futures_util::{SinkExt, StreamExt};
use randomizer::{
    build_router,
    error::ErrorResponse,
    generation::{GENERATOR_VERSION, GenerationMode, GenerationResult},
    standard::ValidationReport,
};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;

#[tokio::test]
async fn rest_generates_json_from_valid_schema() {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {"type": "int", "min": 5, "max": 5}
        }
    });
    let response = build_router(4)
        .oneshot(json_request(schema.clone()))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let result: GenerationResult = serde_json::from_value(response_json(response).await).unwrap();
    assert_eq!(result.value, json!({"id": 5}));
    assert_eq!(result.metadata.sequence, 0);
    assert_eq!(result.metadata.generator_version, GENERATOR_VERSION);
    assert_eq!(result.metadata.contract_hash.len(), 64);

    let replay = build_router(4)
        .oneshot(json_request(json!({
            "schema": schema,
            "seed": result.metadata.seed,
            "sequence": result.metadata.sequence,
            "generator_version": result.metadata.generator_version.clone(),
            "contract_hash": result.metadata.contract_hash.clone()
        })))
        .await
        .unwrap();
    let replay: GenerationResult = serde_json::from_value(response_json(replay).await).unwrap();
    assert_eq!(result, replay);
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

    let unknown_replay_field = build_router(4)
        .oneshot(json_request(json!({
            "schema": {"type": "int"},
            "unknown": true
        })))
        .await
        .unwrap();
    assert_eq!(unknown_replay_field.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rest_replays_exactly_from_returned_metadata() {
    let request = json!({
        "schema": {
            "type": "uuid",
            "prefix": "evt_"
        },
        "seed": 12345,
        "sequence": 9,
        "generator_version": GENERATOR_VERSION
    });
    let first = build_router(4)
        .oneshot(json_request(request.clone()))
        .await
        .unwrap();
    let replay = build_router(4)
        .oneshot(json_request(request))
        .await
        .unwrap();

    let first: GenerationResult = serde_json::from_value(response_json(first).await).unwrap();
    let replay: GenerationResult = serde_json::from_value(response_json(replay).await).unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.metadata.seed, 12345);
    assert_eq!(first.metadata.sequence, 9);
}

#[tokio::test]
async fn rest_generates_and_validates_standard_contracts() {
    let request = json!({
        "contract": standard_contract_json(),
        "mode": "valid",
        "seed": 2024,
        "sequence": 3
    });
    let response = build_router(4)
        .oneshot(json_request(request.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let generated: GenerationResult =
        serde_json::from_value(response_json(response).await).unwrap();
    assert_eq!(generated.mode, GenerationMode::Valid);
    assert_eq!(generated.contract.as_ref().unwrap().name, "api-event");

    let validation = build_router(4)
        .oneshot(json_request_at(
            "/validate",
            json!({
                "contract": standard_contract_json(),
                "value": generated.value
            }),
        ))
        .await
        .unwrap();
    let report: ValidationReport = serde_json::from_value(response_json(validation).await).unwrap();
    assert!(report.valid);

    let invalid = build_router(4)
        .oneshot(json_request(json!({
            "contract": standard_contract_json(),
            "mode": "invalid",
            "seed": 2024,
            "sequence": 3
        })))
        .await
        .unwrap();
    let invalid: GenerationResult = serde_json::from_value(response_json(invalid).await).unwrap();
    assert_eq!(invalid.mode, GenerationMode::Invalid);
    assert!(invalid.violated_rule.is_some());
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
                "seed": 777,
                "sequence": 5,
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
    let result: GenerationResult = serde_json::from_str(&message.into_text().unwrap()).unwrap();
    assert_eq!(result.value, json!(9));
    assert_eq!(result.metadata.seed, 777);
    assert_eq!(result.metadata.sequence, 5);
    assert_eq!(result.metadata.generator_version, GENERATOR_VERSION);

    let next = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let next: GenerationResult = serde_json::from_str(&next.into_text().unwrap()).unwrap();
    assert_eq!(next.metadata.seed, 777);
    assert_eq!(next.metadata.sequence, 6);
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

#[tokio::test]
async fn websocket_streams_reproducible_standard_contract_results() {
    let (address, server) = spawn_server().await;
    let (mut socket, _) = connect_async(format!("ws://{address}/stream"))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            json!({
                "frequency": 100,
                "contract": standard_contract_json(),
                "mode": "maximum",
                "seed": 81,
                "sequence": 12
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

    let message = socket.next().await.unwrap().unwrap().into_text().unwrap();
    let result: GenerationResult = serde_json::from_str(&message).unwrap();
    assert_eq!(result.metadata.seed, 81);
    assert_eq!(result.metadata.sequence, 12);
    assert_eq!(result.mode, GenerationMode::Maximum);
    assert!(result.contract.is_some());

    server.abort();
}

fn json_request(body: Value) -> Request<Body> {
    json_request_at("/generate", body)
}

fn json_request_at(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn standard_contract_json() -> Value {
    json!({
        "name": "api-event",
        "version": "1.0.0",
        "source": "inline:test",
        "schema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["id", "email"],
            "properties": {
                "id": {"type": "string", "format": "uuid"},
                "email": {"type": "string", "format": "email"},
                "note": {"type": "string", "minLength": 2, "maxLength": 5}
            },
            "additionalProperties": false
        }
    })
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
