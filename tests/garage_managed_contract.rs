use std::{fs, path::Path, process::Command};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use randomizer::{
    build_project_router,
    mock::CompiledMockRegistry,
    project::{ProjectPaths, validate_manifest},
    schema::JsonSchemaContract,
    standard::ImportedContract,
};
use serde_json::{Value, json};
use tower::ServiceExt;

const TASK_SLUGS: [&str; 4] = [
    "acko_garage_drop_job_7",
    "acko_garage_drop_task_7",
    "acko_garage_pickup_job_10",
    "acko_garage_pickup_task_10",
];
const CURRENT_STATES: [&str; 2] = ["IN_PROGRESS", "DONE"];

#[tokio::test]
async fn serves_garage_task_eta_from_a_managed_contract_across_request_sequences() {
    let directory = tempfile::tempdir().unwrap();
    let paths = initialize(directory.path());
    fs::create_dir(directory.path().join("specs")).unwrap();
    fs::write(
        directory.path().join("specs/task-eta.schema.json"),
        serde_json::to_vec_pretty(&task_eta_schema()).unwrap(),
    )
    .unwrap();
    let root = directory.path().to_str().unwrap();

    let imported = run_randomizer(&[
        "contract",
        "import",
        "get-task-eta-200",
        "--project",
        root,
        "--source",
        "specs/task-eta.schema.json",
        "--format",
        "json-schema",
        "--method",
        "GET",
        "--endpoint",
        "/api/v1/task/{taskId}/eta",
        "--status",
        "200",
        "--root-symbol",
        "ServiceOSStdResponse<TaskEtaResponseDto>",
    ]);
    assert_command_succeeded(&imported);

    fs::write(
        &paths.manifest,
        r#"version: 2
project:
  name: garage-task-eta
  seed: 20260826
  host: 127.0.0.1
  port: 7263
services:
  - id: serviceos-task-eta
routes:
  - id: get-task-eta
    service: serviceos-task-eta
    match:
      method: GET
      path: /api/v1/task/{taskId}/eta
    responses:
      - status: 200
        body:
          contract: .randomizer/contracts/get-task-eta-200.json
          mode: valid
        bindings:
          - target: /data/task_id
            source: '${request.path.taskId}'
            coerce: integer
"#,
    )
    .unwrap();

    let verified = run_randomizer(&[
        "verify",
        "--project",
        root,
        "--require-managed-contract-route",
        "get-task-eta",
    ]);
    assert_command_succeeded(&verified);
    assert!(
        String::from_utf8_lossy(&verified.stdout).contains("1 managed contracts"),
        "{}",
        String::from_utf8_lossy(&verified.stdout)
    );

    let artifact: JsonSchemaContract =
        serde_json::from_slice(&fs::read(paths.contracts.join("get-task-eta-200.json")).unwrap())
            .unwrap();
    assert_eq!(
        artifact
            .schema
            .pointer("/properties/data/properties/task_slug/enum"),
        Some(&json!(TASK_SLUGS))
    );
    assert_eq!(
        artifact
            .schema
            .pointer("/properties/data/properties/current_state/enum"),
        Some(&json!(CURRENT_STATES))
    );
    for pointer in [
        "/properties/data/properties/task_slug/enum",
        "/properties/data/properties/current_state/enum",
    ] {
        assert!(
            artifact
                .schema
                .pointer(pointer)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .all(|value| !value.is_null()),
            "enum at {pointer} must not admit null"
        );
    }
    let contract = ImportedContract::import(artifact).unwrap();

    let manifest = paths.load_manifest().unwrap();
    validate_manifest(&manifest).unwrap();
    let registry = CompiledMockRegistry::compile(&manifest, &paths).unwrap();
    let router = build_project_router(16, registry);

    let first = get_task_eta(router.clone(), 4_242).await;
    let second = get_task_eta(router.clone(), 4_242).await;
    let other_task = get_task_eta(router.clone(), 9_001).await;

    assert_task_eta_body(&contract, &first, 4_242);
    assert_task_eta_body(&contract, &second, 4_242);
    assert_task_eta_body(&contract, &other_task, 9_001);
    assert_ne!(
        first, second,
        "successive requests must advance the deterministic generation sequence"
    );

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

    let replay = get_task_eta(router, 4_242).await;
    assert_eq!(replay, first, "reset must replay sequence zero exactly");
    assert_task_eta_body(&contract, &replay, 4_242);
}

fn run_randomizer(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_randomizer"))
        .args(arguments)
        .output()
        .unwrap()
}

fn initialize(root: &Path) -> ProjectPaths {
    let initialized = run_randomizer(&["init", root.to_str().unwrap()]);
    assert_command_succeeded(&initialized);
    ProjectPaths::discover(Some(root)).unwrap()
}

fn assert_command_succeeded(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn get_task_eta(router: axum::Router, task_id: u64) -> Value {
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/mock/serviceos-task-eta/api/v1/task/{task_id}/eta"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn assert_task_eta_body(contract: &ImportedContract, body: &Value, expected_task_id: u64) {
    let report = contract.validate(body);
    assert!(report.valid, "schema violations: {:?}", report.violations);

    let data = body.get("data").and_then(Value::as_object).unwrap();
    assert_eq!(
        data.get("task_id").and_then(Value::as_u64),
        Some(expected_task_id),
        "the path value must be bound as a JSON integer"
    );

    let task_slug = data.get("task_slug").and_then(Value::as_str).unwrap();
    assert!(TASK_SLUGS.contains(&task_slug));
    let current_state = data.get("current_state").and_then(Value::as_str).unwrap();
    assert!(CURRENT_STATES.contains(&current_state));

    let remaining = data
        .get("total_remaining_minutes")
        .and_then(Value::as_str)
        .unwrap();
    assert!(!remaining.is_empty());
    assert!(remaining.bytes().all(|byte| byte.is_ascii_digit()));
    assert!(remaining.parse::<u16>().is_ok());

    for field in ["predicted_completion_time", "trip_started_time"] {
        let timestamp = data.get(field).and_then(Value::as_str).unwrap();
        assert!(
            timestamp.contains('T'),
            "{field} is not a date-time: {timestamp}"
        );
        assert!(
            timestamp.ends_with('Z') || timestamp.rfind(['+', '-']).is_some_and(|index| index > 9),
            "{field} has no explicit timezone: {timestamp}"
        );
    }
}

fn task_eta_schema() -> Value {
    // This explicit test contract exercises the runtime capabilities Garage needs. The bundled
    // skill separately owns evidence gathering and must not copy these test-only constraints into
    // an application repository without corroborating them there.
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["data"],
        "properties": {
            "data": {
                "type": "object",
                "required": [
                    "task_id",
                    "task_slug",
                    "predicted_completion_time",
                    "total_remaining_minutes",
                    "current_state",
                    "trip_started_time"
                ],
                "properties": {
                    "task_id": {"type": "integer", "minimum": 1},
                    "task_slug": {"type": "string", "enum": TASK_SLUGS},
                    "predicted_completion_time": {"type": "string", "format": "date-time"},
                    "total_remaining_minutes": {
                        "type": "string",
                        "pattern": "^[0-9]{1,3}$"
                    },
                    "current_state": {"type": "string", "enum": CURRENT_STATES},
                    "trip_started_time": {"type": "string", "format": "date-time"}
                },
                "additionalProperties": false
            }
        },
        "additionalProperties": false
    })
}
