use randomizer::provider::{
    EVIDENCE_KIND_ENUM, EVIDENCE_KIND_FORMAT, EVIDENCE_KIND_PROPERTY_NAME,
    EVIDENCE_KIND_REQUIREDNESS, EVIDENCE_KIND_RESPONSE_WRAPPER, EVIDENCE_KIND_ROOT_SYMBOL,
    EVIDENCE_KIND_TYPE, EndpointSelector, FieldEvidence, JSON_SCHEMA_DRAFT_2020_12,
    PROVIDER_PROTOCOL_VERSION, ProviderIdentity, ProviderRequest, ProviderResponse,
    fingerprint_bytes, import_openapi_response_bytes, validate_provider_response,
};
use serde_json::json;

#[test]
fn public_protocol_request_is_language_neutral_and_versioned() {
    let mut request = ProviderRequest::new(
        EndpointSelector::new("GET", "/v1/accounts/{id}", 200).with_media_type("application/json"),
    );
    request.root_symbol = Some("AccountResponse".to_string());
    request.source_paths = vec!["src/contracts/account.ts".to_string()];

    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "protocol_version": PROVIDER_PROTOCOL_VERSION,
            "endpoint": {
                "method": "GET",
                "path": "/v1/accounts/{id}",
                "status": 200,
                "media_type": "application/json"
            },
            "root_symbol": "AccountResponse",
            "source_paths": ["src/contracts/account.ts"]
        })
    );
}

#[test]
fn public_openapi_import_preserves_response_datatypes() {
    let document = br#"
openapi: 3.1.0
info: {title: Accounts, version: '1'}
paths:
  /v1/accounts/{id}:
    get:
      responses:
        '200':
          content:
            application/json:
              schema:
                type: object
                required: [status, created_at]
                properties:
                  status: {type: string, enum: [OPEN, CLOSED]}
                  created_at: {type: string, format: date-time}
"#;

    let imported = import_openapi_response_bytes(
        "openapi.yaml",
        document,
        EndpointSelector::new("GET", "/v1/accounts/{id}", 200),
        Some("AccountResponse".to_string()),
    )
    .unwrap();

    assert_eq!(imported.schema["$schema"], JSON_SCHEMA_DRAFT_2020_12);
    assert_eq!(
        imported.schema["properties"]["status"]["enum"],
        json!(["OPEN", "CLOSED"])
    );
    assert_eq!(
        imported.schema["properties"]["created_at"]["format"],
        "date-time"
    );
    assert_eq!(imported.root_symbol.as_deref(), Some("AccountResponse"));
    assert_eq!(
        imported.endpoint.media_type.as_deref(),
        Some("application/json")
    );
    assert_eq!(imported.source_fingerprints[0].algorithm, "sha256");

    for (schema_path, kind) in [
        ("#", EVIDENCE_KIND_RESPONSE_WRAPPER),
        ("#", EVIDENCE_KIND_ROOT_SYMBOL),
        ("#/type", EVIDENCE_KIND_TYPE),
        ("#/required/0", EVIDENCE_KIND_REQUIREDNESS),
        ("#/required/1", EVIDENCE_KIND_REQUIREDNESS),
        ("#/properties/status", EVIDENCE_KIND_PROPERTY_NAME),
        ("#/properties/status/type", EVIDENCE_KIND_TYPE),
        ("#/properties/status/enum", EVIDENCE_KIND_ENUM),
        ("#/properties/created_at/format", EVIDENCE_KIND_FORMAT),
    ] {
        assert!(
            imported
                .evidence
                .iter()
                .any(|evidence| evidence.schema_path == schema_path && evidence.kind == kind),
            "missing {kind} evidence at {schema_path}"
        );
    }
}

#[test]
fn public_protocol_rejects_generic_evidence_for_a_specialized_claim() {
    let mut response = ProviderResponse::new(
        ProviderIdentity::new("example.provider", "1"),
        EndpointSelector::new("GET", "/accounts", 200),
        json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "type": "object",
            "properties": {
                "status": {"type": "string", "enum": ["OPEN", "CLOSED"]}
            }
        }),
        vec![fingerprint_bytes("account.ts", b"source")],
    );
    response.evidence = [
        ("#", EVIDENCE_KIND_RESPONSE_WRAPPER),
        ("#/type", EVIDENCE_KIND_TYPE),
        ("#/properties/status", EVIDENCE_KIND_PROPERTY_NAME),
        ("#/properties/status", EVIDENCE_KIND_REQUIREDNESS),
        ("#/properties/status/type", EVIDENCE_KIND_TYPE),
        ("#/properties/status/enum", "generic_schema_evidence"),
    ]
    .into_iter()
    .map(|(schema_path, kind)| FieldEvidence {
        schema_path: schema_path.to_string(),
        source_path: "account.ts".to_string(),
        source_location: "Account.status".to_string(),
        kind: kind.to_string(),
    })
    .collect();

    let error = validate_provider_response(&response)
        .unwrap_err()
        .to_string();

    assert!(error.contains("#/properties/status/enum"));
    assert!(error.contains(EVIDENCE_KIND_ENUM));
}

#[test]
fn public_openapi_import_rejects_30_documents() {
    let error = import_openapi_response_bytes(
        "openapi.yaml",
        b"openapi: 3.0.3\ninfo: {title: Test, version: '1'}\npaths: {}\n",
        EndpointSelector::new("GET", "/accounts", 200),
        None,
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("expected OpenAPI 3.1.x"));
}
