use randomizer::{
    generation::{GENERATOR_VERSION, GenerationOptions, GenerationPlan, generate_value},
    schema::generate_schema_from_json_str,
};
use serde_json::json;

fn representative_schema() -> randomizer::schema::Schema {
    generate_schema_from_json_str(
        r#"
        {
          "type": "object",
          "properties": {
            "active": {"type": "boolean", "true_probability": 65},
            "count": {"type": "int", "min": -100, "max": 100},
            "id": {"type": "uuid", "prefix": "evt_", "suffix": "_v1"},
            "items": {
              "type": "list",
              "min_length": 2,
              "max_length": 4,
              "items": {"type": "enum", "values": ["a", "b", 7, true]}
            },
            "name": {
              "type": "string",
              "length": 8,
              "string_type": "alphanumeric",
              "prefix": "user_"
            },
            "score": {"type": "float", "min": -20.0, "max": 20.0, "precision": 3}
          }
        }
        "#,
    )
    .unwrap()
}

#[test]
fn identical_contract_seed_sequence_and_version_reproduce_exact_output() {
    let schema = representative_schema();
    let options = GenerationOptions {
        seed: Some(8_675_309),
        sequence: Some(42),
        generator_version: Some(GENERATOR_VERSION.to_string()),
        contract_hash: None,
    };

    let first = generate_value(&schema, &options).unwrap();
    let replay = generate_value(&schema, &options).unwrap();

    assert_eq!(first, replay);
    assert_eq!(
        first.value,
        json!({
            "active": true,
            "count": 49,
            "id": "evt_fd9fa841-8024-4d34-bb78-8963f2250432_v1",
            "items": ["b", "a", "a", 7],
            "name": "user_gEI4qT68",
            "score": 1.343
        })
    );
    assert_eq!(
        first.metadata.contract_hash,
        "28b86c299f473bce49f5de9ca3112b1fb3ab8f62f5096cb8b4413b086a5b6435"
    );
}

#[test]
fn generated_seed_and_metadata_are_sufficient_for_exact_replay() {
    let schema = representative_schema();
    let original = generate_value(&schema, &GenerationOptions::default()).unwrap();
    let replay_options = GenerationOptions {
        seed: Some(original.metadata.seed),
        sequence: Some(original.metadata.sequence),
        generator_version: Some(original.metadata.generator_version.clone()),
        contract_hash: Some(original.metadata.contract_hash.clone()),
    };

    let replay = generate_value(&schema, &replay_options).unwrap();
    assert_eq!(original, replay);
}

#[test]
fn sequence_events_are_independent_and_replayable_out_of_order() {
    let schema = representative_schema();
    let options = GenerationOptions {
        seed: Some(123_456),
        ..GenerationOptions::default()
    };
    let plan = GenerationPlan::compile(&schema, &options).unwrap();

    let event_10 = plan.generate(10);
    let event_11 = plan.generate(11);
    let replay_10 = plan.generate(10);

    assert_eq!(event_10, replay_10);
    assert_ne!(event_10.value, event_11.value);
    assert_eq!(event_10.metadata.sequence, 10);
    assert_eq!(event_11.metadata.sequence, 11);
}

#[test]
fn deterministic_uuid_is_valid_v4_and_replays_exactly() {
    let schema = generate_schema_from_json_str(r#"{"type":"uuid"}"#).unwrap();
    let options = GenerationOptions {
        seed: Some(99),
        sequence: Some(3),
        ..GenerationOptions::default()
    };

    let first = generate_value(&schema, &options).unwrap();
    let replay = generate_value(&schema, &options).unwrap();
    let uuid = uuid::Uuid::parse_str(first.value.as_str().unwrap()).unwrap();

    assert_eq!(first, replay);
    assert_eq!(uuid.get_version_num(), 4);
}

#[test]
fn supplied_contract_hash_prevents_replay_with_a_different_contract() {
    let schema = representative_schema();
    let error = generate_value(
        &schema,
        &GenerationOptions {
            contract_hash: Some("0".repeat(64)),
            ..GenerationOptions::default()
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("contract_hash does not match"));
}

#[test]
fn unsupported_generator_version_is_rejected_instead_of_silently_changing_output() {
    let schema = representative_schema();
    let error = generate_value(
        &schema,
        &GenerationOptions {
            generator_version: Some("2".to_string()),
            ..GenerationOptions::default()
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("unsupported generator_version"));
}
