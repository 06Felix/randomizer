use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, Utf8Bytes, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
};
use bytes::{BufMut, BytesMut};
use rand::{SeedableRng, rng, rngs::SmallRng};
use serde::Serialize;
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::{Duration, interval};
use tracing::{debug, warn};

use crate::{compiler::compile_schema, schema::model::WsRequest, state::AppState};

/// Frequency interval in milliseconds between WebSocket payloads (`request.frequency`).
const MIN_FREQUENCY_MS: u64 = 100;
const MAX_FREQUENCY_MS: u64 = 10000;

/// JSON envelope for protocol errors sent as WebSocket text frames.
#[derive(Serialize)]
struct WsErrorBody {
    error: String,
}

fn ws_error_frame(message: impl Into<String>) -> Utf8Bytes {
    let body = WsErrorBody {
        error: message.into(),
    };
    match serde_json::to_string(&body) {
        Ok(s) => s.into(),
        Err(_) => r#"{"error":"failed to encode error message"}"#.into(),
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
                axum::Json(WsErrorBody {
                    error: "maximum concurrent streaming connections reached".to_string(),
                }),
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
                    .send(Message::Text(ws_error_frame(format!("Invalid JSON: {e}"))))
                    .await;
                return;
            }
        },
        message => {
            let _ = socket
                .send(Message::Text(ws_error_frame("Expected a schema config")))
                .await;
            warn!(message = ?message, "unexpected first websocket message");
            return;
        }
    };

    let frequency = request.frequency;
    if frequency < MIN_FREQUENCY_MS || frequency > MAX_FREQUENCY_MS {
        let _ = socket
            .send(Message::Text(ws_error_frame(format!(
                "frequency must be between {MIN_FREQUENCY_MS} ms and {MAX_FREQUENCY_MS} ms"
            ))))
            .await;
        return;
    }

    let generator = match compile_schema(&request.schema) {
        Ok(gnr) => {
            debug!("compiled websocket generator");
            gnr
        }
        Err(e) => {
            warn!(error = %e, "websocket schema compilation failed");
            let _ = socket.send(Message::Text(ws_error_frame(e))).await;
            return;
        }
    };

    let mut ticker = interval(Duration::from_millis(frequency));
    let mut rng = SmallRng::from_rng(&mut rng());
    let mut json_buf = BytesMut::with_capacity(256);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let value = generator.generate(&mut rng);
                debug!(response = %value, "sending websocket value");

                json_buf.clear();
                let mut writer = (&mut json_buf).writer();
                if serde_json::to_writer(&mut writer, &value).is_err() {
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
