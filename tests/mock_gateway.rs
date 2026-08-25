use std::{collections::BTreeMap, fs};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use randomizer::{
    build_project_router,
    mock::CompiledMockRegistry,
    project::{
        BindingDefinition, CURRENT_MANIFEST_VERSION, MatchDefinition, ProjectDefinition,
        ProjectManifest, ProjectPaths, ResponseBodyDefinition, ResponseDefinition, RouteDefinition,
        ServiceDefinition,
    },
};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn serves_normal_third_party_request_and_binds_path_values() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ProjectPaths::for_init(directory.path()).unwrap();
    let manifest = manifest(vec![ResponseDefinition {
        status: 201,
        headers: BTreeMap::from([("x-mock".into(), "randomizer".into())]),
        delay_ms: 0,
        body: ResponseBodyDefinition {
            inline: Some(json!({"data": {"id": "unset", "status": "created"}})),
            ..ResponseBodyDefinition::default()
        },
        bindings: vec![BindingDefinition {
            target: "/data/id".into(),
            source: "${request.path.taskId}".into(),
        }],
    }]);
    let registry = CompiledMockRegistry::compile(&manifest, &paths).unwrap();
    let response = build_project_router(4, registry)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mock/service-os/api/v1/tasks/42?include=details")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["x-mock"], "randomizer");
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body, json!({"data": {"id": "42", "status": "created"}}));
}

#[tokio::test]
async fn advances_scenario_then_holds_the_last_response_and_can_reset() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ProjectPaths::for_init(directory.path()).unwrap();
    let responses = ["first", "second"]
        .into_iter()
        .map(|value| ResponseDefinition {
            status: 200,
            headers: BTreeMap::new(),
            delay_ms: 0,
            body: ResponseBodyDefinition {
                inline: Some(json!({"value": value})),
                ..ResponseBodyDefinition::default()
            },
            bindings: Vec::new(),
        })
        .collect();
    let registry = CompiledMockRegistry::compile(&manifest(responses), &paths).unwrap();
    let router = build_project_router(4, registry);

    assert_eq!(mock_value(router.clone()).await, "first");
    assert_eq!(mock_value(router.clone()).await, "second");
    assert_eq!(mock_value(router.clone()).await, "second");

    let reset = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/__randomizer/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset.status(), StatusCode::NO_CONTENT);
    assert_eq!(mock_value(router).await, "first");
}

#[tokio::test]
async fn matches_request_metadata_and_binds_all_supported_sources() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ProjectPaths::for_init(directory.path()).unwrap();
    let mut manifest = manifest(vec![ResponseDefinition {
        status: 200,
        headers: BTreeMap::new(),
        delay_ms: 0,
        body: ResponseBodyDefinition {
            inline: Some(json!({
                "path": null,
                "query": null,
                "header": null,
                "body": null
            })),
            ..ResponseBodyDefinition::default()
        },
        bindings: vec![
            BindingDefinition {
                target: "/path".into(),
                source: "${request.path.taskId}".into(),
            },
            BindingDefinition {
                target: "/query".into(),
                source: "${request.query.include}".into(),
            },
            BindingDefinition {
                target: "/header".into(),
                source: "${request.header.x-client}".into(),
            },
            BindingDefinition {
                target: "/body".into(),
                source: "${request.body./payload/id}".into(),
            },
        ],
    }]);
    manifest.routes[0].request_match.method = Some("POST".into());
    manifest.routes[0].request_match.headers =
        BTreeMap::from([("x-client".into(), "garage".into())]);
    manifest.routes[0].request_match.body =
        BTreeMap::from([("/payload/type".into(), json!("task"))]);
    let registry = CompiledMockRegistry::compile(&manifest, &paths).unwrap();
    let router = build_project_router(4, registry);

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mock/service-os/api/v1/tasks/42?include=details")
                .header("X-Client", "garage")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"payload": {"type": "task", "id": 99}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        body,
        json!({"path": "42", "query": "details", "header": "garage", "body": 99})
    );
}

#[tokio::test]
async fn generates_enum_boolean_and_collection_fields_from_a_bare_contract() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ProjectPaths::for_init(directory.path()).unwrap();
    fs::create_dir_all(&paths.contracts).unwrap();
    let contract_path = paths.contracts.join("task-state.json");
    fs::write(
        &contract_path,
        serde_json::to_vec_pretty(&json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["status", "active", "retryable", "tags", "note"],
            "properties": {
                "status": {"type": "string", "enum": ["QUEUED", "IN_PROGRESS", "DONE"]},
                "active": {"type": "boolean"},
                "retryable": {"const": false},
                "tags": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 2,
                    "items": {"type": "string", "enum": ["PRIMARY", "SECONDARY"]}
                },
                "note": {"type": ["string", "null"], "minLength": 2}
            },
            "additionalProperties": false
        }))
        .unwrap(),
    )
    .unwrap();
    let registry = CompiledMockRegistry::compile(
        &manifest(vec![ResponseDefinition {
            status: 200,
            headers: BTreeMap::new(),
            delay_ms: 0,
            body: ResponseBodyDefinition {
                contract: Some(".randomizer/contracts/task-state.json".into()),
                ..ResponseBodyDefinition::default()
            },
            bindings: Vec::new(),
        }]),
        &paths,
    )
    .unwrap();

    let response = build_project_router(4, registry)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mock/service-os/api/v1/tasks/42?include=details")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();

    assert!(["QUEUED", "IN_PROGRESS", "DONE"].contains(&body["status"].as_str().unwrap()));
    assert!(body["active"].is_boolean());
    assert_eq!(body["retryable"], false);
    assert!((1..=2).contains(&body["tags"].as_array().unwrap().len()));
    assert!(
        body["tags"]
            .as_array()
            .unwrap()
            .iter()
            .all(|tag| { ["PRIMARY", "SECONDARY"].contains(&tag.as_str().unwrap()) })
    );
    assert!(body["note"].is_string() || body["note"].is_null());
}

#[test]
fn rejects_contract_features_that_cannot_generate_during_verification() {
    let directory = tempfile::tempdir().unwrap();
    let paths = ProjectPaths::for_init(directory.path()).unwrap();
    fs::create_dir_all(&paths.contracts).unwrap();
    fs::write(
        paths.contracts.join("unsupported.json"),
        serde_json::to_vec_pretty(&json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "string",
            "format": "hostname"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = CompiledMockRegistry::compile(
        &manifest(vec![ResponseDefinition {
            status: 200,
            headers: BTreeMap::new(),
            delay_ms: 0,
            body: ResponseBodyDefinition {
                contract: Some(".randomizer/contracts/unsupported.json".into()),
                ..ResponseBodyDefinition::default()
            },
            bindings: Vec::new(),
        }]),
        &paths,
    )
    .err()
    .unwrap()
    .to_string();

    assert!(error.contains("unsupported format \"hostname\""));
}

fn manifest(responses: Vec<ResponseDefinition>) -> ProjectManifest {
    ProjectManifest {
        version: CURRENT_MANIFEST_VERSION,
        project: ProjectDefinition {
            name: "test".into(),
            seed: 7,
            host: "127.0.0.1".into(),
            port: 7263,
            adapter: None,
        },
        services: vec![ServiceDefinition {
            id: "service-os".into(),
            config_key: None,
            wiring: Vec::new(),
        }],
        routes: vec![RouteDefinition {
            id: "get-task".into(),
            service: "service-os".into(),
            request_match: MatchDefinition {
                method: Some("GET".into()),
                path: "/api/v1/tasks/{taskId}".into(),
                query: BTreeMap::from([("include".into(), "details".into())]),
                headers: BTreeMap::new(),
                body: BTreeMap::new(),
            },
            responses,
        }],
    }
}

async fn mock_value(router: axum::Router) -> String {
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mock/service-os/api/v1/tasks/1?include=details")
                .header(header::ACCEPT, "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    body["value"].as_str().unwrap().to_string()
}
