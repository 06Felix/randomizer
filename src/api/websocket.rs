use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, Utf8Bytes, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
};
use bytes::{BufMut, BytesMut};
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::{Duration, interval};
use tracing::{debug, warn};

use crate::{
    error::{ErrorResponse, GenerationError},
    generation::GenerationPlan,
    schema::model::WsRequest,
    state::AppState,
};

/// Frequency interval in milliseconds between WebSocket payloads (`request.frequency`).
const MIN_FREQUENCY_MS: u64 = 100;
const MAX_FREQUENCY_MS: u64 = 10000;

/// JSON envelope for protocol errors sent as WebSocket text frames.
fn ws_error_frame(code: &'static str, message: impl Into<String>) -> Utf8Bytes {
    let body = ErrorResponse::new(code, message);
    match serde_json::to_string(&body) {
        Ok(s) => s.into(),
        Err(_) => r#"{"code":"internal_error","message":"failed to encode error message"}"#.into(),
    }
}

/// Upgrades an HTTP request to a WebSocket stream of generated JSON values.
pub async fn stream(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    debug!("received websocket upgrade request");

    let permit = match state.ws_connection_limit.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            warn!("rejected websocket: concurrent streaming connection limit reached");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(ErrorResponse::new(
                    "capacity_exceeded",
                    "maximum concurrent streaming connections reached",
                )),
            )
                .into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(socket, permit))
}

/// Handles one WebSocket client by reading a schema and streaming values on an interval.
async fn handle_socket(mut socket: WebSocket, _permit: OwnedSemaphorePermit) {
    debug!("websocket connection established");
    let request = match socket.recv().await {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<WsRequest>(&text) {
            Ok(schema) => {
                debug!(schema = ?schema, "received websocket schema");
                schema
            }
            Err(e) => {
                warn!(error = %e, "invalid websocket schema payload");
                let _ = socket
                    .send(Message::Text(ws_error_frame(
                        "invalid_request",
                        format!("invalid JSON: {e}"),
                    )))
                    .await;
                return;
            }
        },
        message => {
            let _ = socket
                .send(Message::Text(ws_error_frame(
                    "invalid_request",
                    "expected a schema config",
                )))
                .await;
            warn!(message = ?message, "unexpected first websocket message");
            return;
        }
    };

    let frequency = request.frequency;
    if !(MIN_FREQUENCY_MS..=MAX_FREQUENCY_MS).contains(&frequency) {
        let _ = socket
            .send(Message::Text(ws_error_frame(
                "invalid_frequency",
                format!(
                    "frequency must be between {MIN_FREQUENCY_MS} ms and {MAX_FREQUENCY_MS} ms"
                ),
            )))
            .await;
        return;
    }

    let generation_options = request.generation_options();
    let mut sequence = generation_options.sequence.unwrap_or(0);
    let plan = match GenerationPlan::compile(&request.schema, &generation_options) {
        Ok(plan) => {
            debug!(
                seed = plan.seed(),
                contract_hash = plan.contract_hash(),
                "compiled websocket generation plan"
            );
            plan
        }
        Err(e) => {
            warn!(error = %e, "websocket generation plan failed");
            let code = match &e {
                GenerationError::InvalidSchema(_) => "invalid_schema",
                GenerationError::UnsupportedGeneratorVersion { .. } => {
                    "unsupported_generator_version"
                }
                GenerationError::ContractHashMismatch { .. } => "contract_hash_mismatch",
                GenerationError::Canonicalization(_) => "internal_error",
            };
            let _ = socket
                .send(Message::Text(ws_error_frame(code, e.to_string())))
                .await;
            return;
        }
    };

    let mut ticker = interval(Duration::from_millis(frequency));
    let mut json_buf = BytesMut::with_capacity(256);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let result = plan.generate(sequence);
                debug!(response = %result.value, metadata = ?result.metadata, "sending websocket value");

                json_buf.clear();
                let mut writer = (&mut json_buf).writer();
                if serde_json::to_writer(&mut writer, &result).is_err() {
                    debug!("failed to serialize websocket value");
                    break;
                }

                // Move the written bytes into the websocket frame without copying.
                let payload = json_buf.split().freeze();
                let text = match Utf8Bytes::try_from(payload) {
                    Ok(t) => t,
                    Err(_) => {
                        debug!("serialized JSON was not valid utf-8");
                        break;
                    }
                };

                if socket.send(Message::Text(text)).await.is_err() {
                    debug!("websocket client disconnected during send");
                    break;
                }

                let Some(next_sequence) = sequence.checked_add(1) else {
                    debug!("maximum sequence emitted; closing websocket stream");
                    break;
                };
                sequence = next_sequence;
            }

            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => {
                        debug!("websocket client disconnected");
                        break;
                    }
                    message => {
                        debug!(message = ?message, "ignoring websocket control/message while streaming");
                    }
                }
            }
        }
    }
}
