use axum::{
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use rand::{SeedableRng, rng, rngs::SmallRng};
use tokio::time::{Duration, interval};
use tracing::{debug, warn};

use crate::{compiler::compile_schema, schema::Schema};

/// Upgrades an HTTP request to a WebSocket stream of generated JSON values.
pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    debug!("received websocket upgrade request");
    ws.on_upgrade(handle_socket)
}

/// Handles one WebSocket client by reading a schema and streaming values on an interval.
async fn handle_socket(mut socket: WebSocket) {
    debug!("websocket connection established");
    let schema = match socket.recv().await {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<Schema>(&text) {
            Ok(schema) => {
                debug!(schema = ?schema, "received websocket schema");
                schema
            }
            Err(e) => {
                warn!(error = %e, "invalid websocket schema payload");
                let _ = socket
                    .send(Message::Text(
                        format!(r#"{{"error": "Invalid JSON: {e}"}}"#).into(),
                    ))
                    .await;
                return;
            }
        },
        message => {
            let _ = socket
                .send(Message::Text(
                    r#"{"error": "Expected a schema config"}"#.into(),
                ))
                .await;
            warn!(message = ?message, "unexpected first websocket message");
            return;
        }
    };

    let generator = match compile_schema(&schema) {
        Ok(g) => {
            debug!("compiled websocket generator");
            g
        }
        Err(e) => {
            warn!(error = %e, "websocket schema compilation failed");
            let _ = socket
                .send(Message::Text(format!(r#"{{"error": "{}"}}"#, e).into()))
                .await;
            return;
        }
    };

    let mut ticker = interval(Duration::from_secs(1));
    let mut rng = SmallRng::from_rng(&mut rng());

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let value = generator.generate(&mut rng);
                debug!(response = %value, "sending websocket value");
                if socket.send(Message::Text(
                    serde_json::to_string(&value).unwrap().into()
                )).await.is_err() {
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
                    message => {debug!(message = ?message, "ignoring websocket control/message while streaming");} // ignore other messages while streaming
                }
            }
        }
    }
}
