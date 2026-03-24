use axum::{extract::{WebSocketUpgrade, ws::{Message, WebSocket}}, response::IntoResponse};
use rand::{SeedableRng, rng, rngs::SmallRng};
use tokio::time::{interval, Duration};

use crate::{compiler::compile_schema, schema::Schema};

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    let schema = match socket.recv().await {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<Schema>(&text) {
                Ok(scema) => scema,
                Err(e) => {
                    let _ = socket.send(Message::Text(
                        format!(r#"{{"error": "Invalid JSON: {e}"}}"#).into()
                    )).await;
                    return;
                }
            }
        }
        _ => {
            let _ = socket.send(Message::Text(
                r#"{"error": "Expected a schema config"}"#.into()
            )).await;
            return;
        }
    };

    println!("Client requested: {:?}", schema);
    let generator = match compile_schema(&schema) {
        Ok(g) => g,
        Err(e) => {
            let _ = socket.send(Message::Text(
                format!(r#"{{"error": "{}"}}"#, e).into(),
            )).await;
            return;
        }
    };

    let mut ticker = interval(Duration::from_secs(1));
    let mut rng = SmallRng::from_rng(&mut rng());

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let value = generator.generate(&mut rng);

                if socket.send(Message::Text(
                    serde_json::to_string(&value).unwrap().into()
                )).await.is_err() {
                    println!("Client disconnected");
                    break;
                }
            }

            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => {
                        println!("Client disconnected");
                        break;
                    }
                    _ => {} // ignore other messages while streaming
                }
            }
        }
    }
}