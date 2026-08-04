use randomizer::{
    generation::{GenerationMode, GenerationOptions},
    schema::JsonSchemaContract,
    standard::{ImportedContract, generate_standard_value},
};
use serde_json::{Value, json};

fn contract() -> JsonSchemaContract {
    JsonSchemaContract {
        name: "customer-event".to_string(),
        version: "2.3.0".to_string(),
        source: "contracts/customer-event.json".to_string(),
        content_hash: None,
        schema: json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": {
                "identifier": {"type": "string", "format": "uuid"},
                "status": {"enum": ["created", "active", "closed"]}
            },
            "type": "object",
            "required": [
                "id", "created_at", "event_date", "email", "website", "code",
                "status", "kind", "choice", "flexible", "nullable"
            ],
            "properties": {
                "id": {"$ref": "#/$defs/identifier"},
                "created_at": {"type": "string", "format": "date-time"},
                "event_date": {"type": "string", "format": "date"},
                "email": {"type": "string", "format": "email"},
                "website": {"type": "string", "format": "uri"},
                "code": {"type": "string", "pattern": "^[A-Z]{3}-[0-9]{4}$"},
                "status": {"$ref": "#/$defs/status"},
                "kind": {"const": "customer.updated"},
                "choice": {
                    "oneOf": [
                        {"type": "integer", "minimum": 1, "maximum": 9},
                        {"type": "string", "enum": ["manual"]}
                    ]
                },
                "flexible": {
                    "anyOf": [
                        {"type": "boolean"},
                        {"type": "integer", "minimum": 10, "maximum": 20}
                    ]
                },
                "nullable": {"type": ["string", "null"], "minLength": 2},
                "nickname": {"type": "string", "minLength": 2, "maxLength": 8}
            },
            "additionalProperties": false,
            "examples": [{
                "id": "7d444840-9dc0-11d1-b245-5ffdce74fad2",
                "created_at": "2024-01-01T00:00:00Z",
                "event_date": "2024-01-01",
                "email": "example@example.com",
                "website": "https://example.com",
                "code": "ABC-1234",
                "status": "active",
                "kind": "customer.updated",
                "choice": 1,
                "flexible": true,
                "nullable": null
            }]
        }),
    }
}

fn options() -> GenerationOptions {
    GenerationOptions {
        seed: Some(42),
        sequence: Some(7),
        ..GenerationOptions::default()
    }
}

#[test]
fn imports_validates_and_replays_standard_contracts() {
    let first = generate_standard_value(contract(), GenerationMode::Valid, &options()).unwrap();
    let replay = generate_standard_value(contract(), GenerationMode::Valid, &options()).unwrap();
    let imported = ImportedContract::import(contract()).unwrap();

    assert_eq!(first, replay);
    assert_eq!(
        first.value,
        json!({
            "choice": "manual",
            "code": "EUR-9228",
            "created_at": "2024-01-09T06:52:13Z",
            "email": "user671402@example.com",
            "event_date": "2024-01-03",
            "flexible": 19,
            "id": "82175f20-352d-454b-b7c4-0cc4189dca94",
            "kind": "customer.updated",
            "nullable": "hb2rGQ3hG",
            "status": "active",
            "website": "https://example.com/resources/33809"
        })
    );
    assert_eq!(
        first.metadata.contract_hash,
        "2a9dbe7ef3cc8d5893f419c67bf391b806233ad2d4ce0ef34649b8c884ffa50b"
    );
    assert!(imported.validate(&first.value).valid);
    assert_eq!(first.contract.as_ref().unwrap().name, "customer-event");
    assert_eq!(first.contract.as_ref().unwrap().version, "2.3.0");
    assert_eq!(first.contract.as_ref().unwrap().content_hash.len(), 64);
}

#[test]
fn supports_minimum_maximum_boundary_and_example_modes() {
    let minimum = generate_standard_value(contract(), GenerationMode::Minimum, &options()).unwrap();
    let maximum = generate_standard_value(contract(), GenerationMode::Maximum, &options()).unwrap();
    let boundary =
        generate_standard_value(contract(), GenerationMode::Boundary, &options()).unwrap();
    let example = generate_standard_value(contract(), GenerationMode::Example, &options()).unwrap();
    let imported = ImportedContract::import(contract()).unwrap();

    assert!(minimum.value.get("nickname").is_none());
    assert!(maximum.value.get("nickname").is_some());
    assert_eq!(minimum.value["nullable"], Value::Null);
    assert_eq!(minimum.value["choice"], json!(1));
    assert_eq!(maximum.value["choice"], "manual");
    assert_eq!(maximum.value["status"], "closed");
    assert_eq!(maximum.value["event_date"], "9999-12-31");
    assert!(imported.validate(&minimum.value).valid);
    assert!(imported.validate(&maximum.value).valid);
    assert!(imported.validate(&boundary.value).valid);
    assert_eq!(example.value["code"], "ABC-1234");
    assert!(imported.validate(&example.value).valid);
}

#[test]
fn invalid_mode_returns_the_exact_validator_rule() {
    let generated =
        generate_standard_value(contract(), GenerationMode::Invalid, &options()).unwrap();
    let imported = ImportedContract::import(contract()).unwrap();
    let report = imported.validate(&generated.value);
    let reported = generated.violated_rule.unwrap();

    assert!(!report.valid);
    assert!(report.violations.contains(&reported));
    assert!(!reported.keyword.is_empty());
    assert!(!reported.schema_path.is_empty());
}

#[test]
fn validates_format_and_pattern_failures_with_precise_paths() {
    let imported = ImportedContract::import(contract()).unwrap();
    let invalid = valid_example_with(json!({
        "email": "not-an-email",
        "code": "wrong"
    }));
    let report = imported.validate(&invalid);

    assert!(!report.valid);
    assert!(
        report
            .violations
            .iter()
            .any(|error| { error.keyword == "format" && error.instance_path == "/email" })
    );
    assert!(
        report
            .violations
            .iter()
            .any(|error| { error.keyword == "pattern" && error.instance_path == "/code" })
    );
}

#[test]
fn rejects_external_references_and_bad_content_hashes() {
    let mut external = contract();
    external.schema = json!({"$ref": "https://example.com/schema.json"});
    assert!(
        ImportedContract::import(external)
            .err()
            .unwrap()
            .to_string()
            .contains("external reference")
    );

    let mut mismatched = contract();
    mismatched.content_hash = Some("0".repeat(64));
    assert!(
        ImportedContract::import(mismatched)
            .err()
            .unwrap()
            .to_string()
            .contains("does not match")
    );

    let mut old_draft = contract();
    old_draft.schema["$schema"] = json!("http://json-schema.org/draft-07/schema#");
    assert!(
        ImportedContract::import(old_draft)
            .err()
            .unwrap()
            .to_string()
            .contains("expected Draft 2020-12")
    );
}

fn valid_example_with(overrides: Value) -> Value {
    let mut value = contract().schema["examples"][0].clone();
    if let (Some(target), Some(overrides)) = (value.as_object_mut(), overrides.as_object()) {
        target.extend(overrides.clone());
    }
    value
}
