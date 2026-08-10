use std::collections::BTreeMap;

use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::{error::ErrorResponse, state::AppState};

use super::MockRequest;

pub async fn mock_request(
    State(state): State<AppState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(registry) = state.mock_registry.as_ref() else {
        return error(
            StatusCode::NOT_FOUND,
            "mocking_disabled",
            "no project mock registry is loaded",
        );
    };
    let path = uri.path();
    let Some(remainder) = path.strip_prefix("/mock/") else {
        return error(
            StatusCode::NOT_FOUND,
            "route_not_found",
            "mock route not found",
        );
    };
    let (service, request_path) = remainder
        .split_once('/')
        .map(|(service, path)| (service, format!("/{path}")))
        .unwrap_or((remainder, "/".to_string()));
    let query: BTreeMap<String, String> = uri
        .query()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect()
        })
        .unwrap_or_default();
    let headers = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect();
    let json_body = if body.is_empty() {
        None
    } else {
        match serde_json::from_slice::<Value>(&body) {
            Ok(value) => Some(value),
            Err(source) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "invalid_request_body",
                    format!("request body is not valid JSON: {source}"),
                );
            }
        }
    };
    let request = MockRequest {
        method: method.as_str().to_string(),
        service: service.to_string(),
        path: request_path,
        query,
        headers,
        body: json_body,
    };

    match registry.respond(&request, &state.scenarios) {
        Ok(Some(mock)) => {
            if mock.delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(mock.delay_ms)).await;
            }
            let status =
                StatusCode::from_u16(mock.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let mut response = match mock.body {
                Some(value) => (status, Json(value)).into_response(),
                None => status.into_response(),
            };
            for (name, value) in &mock.headers {
                match (HeaderName::try_from(name), HeaderValue::try_from(value)) {
                    (Ok(name), Ok(value)) => {
                        response.headers_mut().insert(name, value);
                    }
                    _ => {
                        warn!(route_id = %mock.route_id, header = %name, "ignored invalid configured response header");
                    }
                }
            }
            state
                .request_log
                .record(Some(&mock.route_id), &request.method, path, mock.status);
            debug!(route_id = %mock.route_id, status = mock.status, "served mock response");
            response
        }
        Ok(None) => {
            state.request_log.record(None, &request.method, path, 404);
            error(
                StatusCode::NOT_FOUND,
                "mock_not_found",
                format!("no mock matched {} {}", request.method, path),
            )
        }
        Err(source) => {
            warn!(error = %source, "mock response failed");
            state.request_log.record(None, &request.method, path, 500);
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "mock_failed",
                source.to_string(),
            )
        }
    }
}

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "mocking": state.mock_registry.is_some(),
        "routes": state.mock_registry.as_ref().map_or(0, |registry| registry.route_ids().len()),
    }))
}

pub async fn list_routes(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "routes": state.mock_registry.as_ref().map_or_else(Vec::new, |registry| registry.route_ids()),
    }))
}

pub async fn list_requests(State(state): State<AppState>) -> Json<Value> {
    Json(serde_json::to_value(state.request_log.entries()).unwrap_or_else(|_| json!([])))
}

pub async fn reset(State(state): State<AppState>) -> StatusCode {
    state.scenarios.reset();
    state.request_log.clear();
    StatusCode::NO_CONTENT
}

fn error(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (status, Json(ErrorResponse::new(code, message))).into_response()
}
