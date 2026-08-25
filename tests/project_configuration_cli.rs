use std::{fs, path::Path, process::Command};

use randomizer::{
    generation::GenerationMode,
    project::{
        MatchDefinition, ProjectPaths, ResponseBodyDefinition, ResponseDefinition, RouteDefinition,
        ServiceBasePathBehavior, ServiceBaseSafety, ServiceDefinition, WiringDefinition,
        WiringFormat, WiringTarget,
    },
    schema::JsonSchemaContract,
};
use serde_json::json;

fn run_randomizer(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_randomizer"))
        .args(arguments)
        .output()
        .unwrap()
}

fn initialize(root: &Path) -> ProjectPaths {
    let initialized = run_randomizer(&["init", root.to_str().unwrap()]);
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    ProjectPaths::discover(Some(root)).unwrap()
}

#[test]
fn imports_refreshes_and_checks_a_language_neutral_contract() {
    let directory = tempfile::tempdir().unwrap();
    let paths = initialize(directory.path());
    fs::create_dir(directory.path().join("specs")).unwrap();
    let source = directory.path().join("specs/user.schema.json");
    fs::write(
        &source,
        serde_json::to_vec_pretty(&schema(&["ACTIVE", "PAUSED"])).unwrap(),
    )
    .unwrap();
    let root = directory.path().to_str().unwrap();

    let imported = run_randomizer(&[
        "contract",
        "import",
        "get-user-200",
        "--project",
        root,
        "--source",
        "specs/user.schema.json",
        "--format",
        "json-schema",
        "--method",
        "GET",
        "--endpoint",
        "/users/{id}",
        "--status",
        "200",
        "--root-symbol",
        "UserEnvelope",
    ]);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert!(String::from_utf8_lossy(&imported.stdout).contains("saved managed contract"));
    assert!(paths.contracts_lock.is_file());

    let artifact: JsonSchemaContract =
        serde_json::from_slice(&fs::read(paths.contracts.join("get-user-200.json")).unwrap())
            .unwrap();
    assert_eq!(
        artifact.schema["properties"]["state"]["enum"],
        json!(["ACTIVE", "PAUSED"])
    );
    assert_eq!(
        artifact.schema["properties"]["created_at"]["format"],
        "date-time"
    );
    let mut manifest = paths.load_manifest().unwrap();
    manifest.services.push(ServiceDefinition {
        id: "users".into(),
        config_key: None,
        wiring: Vec::new(),
    });
    manifest.routes.push(RouteDefinition {
        id: "get-user".into(),
        service: "users".into(),
        request_match: MatchDefinition {
            method: Some("GET".into()),
            path: "/users/{id}".into(),
            ..MatchDefinition::default()
        },
        responses: vec![ResponseDefinition {
            status: 200,
            headers: Default::default(),
            delay_ms: 0,
            body: ResponseBodyDefinition {
                contract: Some(".randomizer/contracts/get-user-200.json".into()),
                ..ResponseBodyDefinition::default()
            },
            bindings: Vec::new(),
        }],
    });
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();

    let checked = run_randomizer(&["contract", "check", "get-user-200", "--project", root]);
    assert!(checked.status.success());

    fs::write(
        &source,
        serde_json::to_vec_pretty(&schema(&["ACTIVE", "PAUSED", "DELETED"])).unwrap(),
    )
    .unwrap();
    let stale = run_randomizer(&["contract", "check", "get-user-200", "--project", root]);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("fingerprint is stale"));

    let refreshed = run_randomizer(&["contract", "refresh", "get-user-200", "--project", root]);
    assert!(
        refreshed.status.success(),
        "{}",
        String::from_utf8_lossy(&refreshed.stderr)
    );

    manifest.routes[0].request_match.method = Some("POST".into());
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let mismatched = run_randomizer(&["verify", "--project", root]);
    assert!(!mismatched.status.success());
    assert!(String::from_utf8_lossy(&mismatched.stderr).contains("locked endpoint"));

    manifest.routes[0].request_match.method = Some("GET".into());
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let checked = run_randomizer(&["verify", "--project", root]);
    assert!(checked.status.success());

    let required = run_randomizer(&[
        "verify",
        "--project",
        root,
        "--require-managed-contract-route",
        "get-user",
    ]);
    assert!(
        required.status.success(),
        "{}",
        String::from_utf8_lossy(&required.stderr)
    );

    manifest.routes[0].responses[0].body.mode = GenerationMode::Example;
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let example_mode = run_randomizer(&[
        "verify",
        "--project",
        root,
        "--require-managed-contract-route",
        "get-user",
    ]);
    assert!(!example_mode.status.success());
    assert!(
        String::from_utf8_lossy(&example_mode.stderr)
            .contains("managed contract in a generating mode")
    );
    manifest.routes[0].responses[0].body.mode = GenerationMode::Valid;
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();

    let missing = run_randomizer(&[
        "verify",
        "--project",
        root,
        "--require-managed-contract-route",
        "missing-route",
    ]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("was not found"));
}

#[cfg(unix)]
#[test]
fn rejects_contract_sources_that_resolve_outside_the_project() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let paths = initialize(directory.path());
    fs::create_dir(directory.path().join("specs")).unwrap();
    fs::write(
        outside.path().join("user.schema.json"),
        serde_json::to_vec_pretty(&schema(&["ACTIVE"])).unwrap(),
    )
    .unwrap();
    symlink(
        outside.path().join("user.schema.json"),
        directory.path().join("specs/user.schema.json"),
    )
    .unwrap();

    let imported = run_randomizer(&[
        "contract",
        "import",
        "get-user-outside",
        "--project",
        directory.path().to_str().unwrap(),
        "--source",
        "specs/user.schema.json",
        "--format",
        "json-schema",
        "--method",
        "GET",
        "--endpoint",
        "/users/{id}",
    ]);

    assert!(!imported.status.success());
    assert!(String::from_utf8_lossy(&imported.stderr).contains("resolves outside project root"));
    assert!(!paths.contracts_lock.exists());
    assert!(!paths.contracts.join("get-user-outside.json").exists());
}

#[test]
fn openapi_import_persists_the_selected_response_media_type() {
    let directory = tempfile::tempdir().unwrap();
    let paths = initialize(directory.path());
    fs::create_dir(directory.path().join("specs")).unwrap();
    fs::write(
        directory.path().join("specs/errors.openapi.yaml"),
        r#"openapi: 3.1.0
info: {title: Errors, version: '1'}
paths:
  /errors:
    get:
      responses:
        '400':
          content:
            application/problem+json:
              schema:
                type: object
                required: [code]
                properties:
                  code: {type: string, enum: [INVALID_REQUEST]}
"#,
    )
    .unwrap();
    let root = directory.path().to_str().unwrap();
    let imported = run_randomizer(&[
        "contract",
        "import",
        "get-error-400",
        "--project",
        root,
        "--source",
        "specs/errors.openapi.yaml",
        "--format",
        "openapi",
        "--method",
        "GET",
        "--endpoint",
        "/errors",
        "--status",
        "400",
    ]);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(&paths.contracts_lock).unwrap()).unwrap();
    assert_eq!(
        lock["contracts"]["get-error-400"]["endpoint"]["media_type"],
        "application/problem+json"
    );

    let mut manifest = paths.load_manifest().unwrap();
    manifest.services.push(ServiceDefinition {
        id: "errors".into(),
        config_key: None,
        wiring: Vec::new(),
    });
    manifest.routes.push(RouteDefinition {
        id: "get-error".into(),
        service: "errors".into(),
        request_match: MatchDefinition {
            method: Some("GET".into()),
            path: "/errors".into(),
            ..MatchDefinition::default()
        },
        responses: vec![ResponseDefinition {
            status: 400,
            headers: Default::default(),
            delay_ms: 0,
            body: ResponseBodyDefinition {
                contract: Some(".randomizer/contracts/get-error-400.json".into()),
                ..ResponseBodyDefinition::default()
            },
            bindings: Vec::new(),
        }],
    });
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();

    let mismatched = run_randomizer(&["verify", "--project", root]);
    assert!(!mismatched.status.success());
    assert!(String::from_utf8_lossy(&mismatched.stderr).contains("response media type"));

    manifest.routes[0].responses[0]
        .headers
        .insert("content-type".into(), "application/problem+json".into());
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let verified = run_randomizer(&["verify", "--project", root]);
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
}

#[test]
fn applies_and_verifies_structured_application_wiring() {
    let directory = tempfile::tempdir().unwrap();
    let paths = initialize(directory.path());
    fs::write(
        directory.path().join(".env.local"),
        "USERS_API_URL=https://users.example\n",
    )
    .unwrap();
    let mut manifest = paths.load_manifest().unwrap();
    manifest.services.push(ServiceDefinition {
        id: "users".into(),
        config_key: None,
        wiring: vec![WiringDefinition {
            file: ".env.local".into(),
            format: WiringFormat::Dotenv,
            selector: "USERS_API_URL".into(),
            target: WiringTarget::ServiceBaseUrl,
            route: None,
            service_base_safety: Some(ServiceBaseSafety::DedicatedSetting),
            service_base_path_behavior: Some(ServiceBasePathBehavior::PreservesPrefix),
        }],
    });
    manifest.routes.push(RouteDefinition {
        id: "get-user".into(),
        service: "users".into(),
        request_match: MatchDefinition {
            method: Some("GET".into()),
            path: "/users/{id}".into(),
            ..MatchDefinition::default()
        },
        responses: vec![ResponseDefinition {
            status: 200,
            headers: Default::default(),
            delay_ms: 0,
            body: ResponseBodyDefinition {
                inline: Some(json!({"id": "example"})),
                ..ResponseBodyDefinition::default()
            },
            bindings: Vec::new(),
        }],
    });
    fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let root = directory.path().to_str().unwrap();

    let static_route = run_randomizer(&[
        "verify",
        "--project",
        root,
        "--require-managed-contract-route",
        "get-user",
    ]);
    assert!(!static_route.status.success());
    assert!(
        String::from_utf8_lossy(&static_route.stderr)
            .contains("has no response backed by a managed contract in a generating mode")
    );

    let stale = run_randomizer(&["wiring", "check", "--project", root]);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("wiring mismatch"));

    let applied = run_randomizer(&["wiring", "apply", "--project", root, "--service", "users"]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(
        fs::read_to_string(directory.path().join(".env.local")).unwrap(),
        "USERS_API_URL=http://127.0.0.1:7263/mock/users\n"
    );

    let checked = run_randomizer(&["verify", "--project", root]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert!(String::from_utf8_lossy(&checked.stdout).contains("1 endpoint wiring entries"));
}

#[cfg(unix)]
#[test]
fn analyzes_and_refreshes_an_external_language_provider() {
    use std::os::unix::fs::PermissionsExt;

    use sha2::{Digest, Sha256};

    let directory = tempfile::tempdir().unwrap();
    let paths = initialize(directory.path());
    fs::create_dir_all(directory.path().join("src/client")).unwrap();
    fs::create_dir_all(directory.path().join("tools")).unwrap();
    let source_contents = b"serialized response type evidence\n";
    fs::write(
        directory.path().join("src/client/user-types.txt"),
        source_contents,
    )
    .unwrap();
    let digest = format!("{:x}", Sha256::digest(source_contents));
    let response = json!({
        "protocol_version": "1",
        "provider": {"name": "example.language-provider", "version": "1.0.0"},
        "endpoint": {"method": "GET", "path": "/users/{id}", "status": 200},
        "root_symbol": "UserEnvelope",
        "schema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["state", "created_at"],
            "properties": {
                "state": {"type": "string", "enum": ["ACTIVE", "PAUSED"]},
                "created_at": {"type": "string", "format": "date-time"}
            }
        },
        "source_fingerprints": [{
            "path": "src/client/user-types.txt",
            "algorithm": "sha256",
            "digest": digest
        }],
        "evidence": [
            evidence("#", "UserEnvelope", "response_wrapper"),
            evidence("#", "UserEnvelope", "root_symbol"),
            evidence("#/type", "UserEnvelope", "type"),
            evidence("#/required/0", "UserEnvelope.state", "requiredness"),
            evidence("#/required/1", "UserEnvelope.createdAt", "requiredness"),
            evidence("#/properties/state", "UserEnvelope.state", "property_name"),
            evidence("#/properties/state/type", "UserEnvelope.state", "type"),
            evidence("#/properties/state/enum", "UserState", "enum"),
            evidence("#/properties/created_at", "UserEnvelope.createdAt", "property_name"),
            evidence("#/properties/created_at/type", "UserEnvelope.createdAt", "type"),
            evidence("#/properties/created_at/format", "UserEnvelope.createdAt", "format")
        ],
        "diagnostics": []
    });
    let provider_path = directory.path().join("tools/provider.sh");
    fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nread _request\nprintf '%s\\n' '{}'\n",
            serde_json::to_string(&response).unwrap()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&provider_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&provider_path, permissions).unwrap();
    let root = directory.path().to_str().unwrap();

    let analyzed = run_randomizer(&[
        "contract",
        "analyze",
        "get-user-provider",
        "--project",
        root,
        "--provider",
        "./tools/provider.sh",
        "--method",
        "GET",
        "--endpoint",
        "/users/{id}",
        "--status",
        "200",
        "--root-symbol",
        "UserEnvelope",
        "--source",
        "src/client/user-types.txt",
    ]);
    assert!(
        analyzed.status.success(),
        "{}",
        String::from_utf8_lossy(&analyzed.stderr)
    );
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(&paths.contracts_lock).unwrap()).unwrap();
    assert_eq!(
        lock["contracts"]["get-user-provider"]["provider"]["name"],
        "example.language-provider"
    );
    assert_eq!(
        lock["contracts"]["get-user-provider"]["evidence"][0]["kind"],
        "response_wrapper"
    );
    assert!(
        lock["contracts"]["get-user-provider"]["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["schema_path"] == "#/properties/state/enum" && item["kind"] == "enum")
    );

    let refreshed = run_randomizer(&[
        "contract",
        "refresh",
        "get-user-provider",
        "--project",
        root,
    ]);
    assert!(
        refreshed.status.success(),
        "{}",
        String::from_utf8_lossy(&refreshed.stderr)
    );
}

fn evidence(schema_path: &str, source_location: &str, kind: &str) -> serde_json::Value {
    json!({
        "schema_path": schema_path,
        "source_path": "src/client/user-types.txt",
        "source_location": source_location,
        "kind": kind
    })
}

fn schema(states: &[&str]) -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["state", "created_at"],
        "properties": {
            "state": {"type": "string", "enum": states},
            "created_at": {"type": "string", "format": "date-time"}
        },
        "additionalProperties": false
    })
}
