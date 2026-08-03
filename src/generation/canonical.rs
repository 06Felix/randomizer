use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{error::GenerationError, schema::Schema};

/// Returns canonical JSON with recursively sorted object keys and no insignificant whitespace.
pub fn canonical_schema_json(schema: &Schema) -> Result<String, GenerationError> {
    let value = serde_json::to_value(schema).map_err(GenerationError::Canonicalization)?;
    serde_json::to_string(&canonicalize(value)).map_err(GenerationError::Canonicalization)
}

pub fn contract_hash(schema: &Schema) -> Result<String, GenerationError> {
    let digest = Sha256::digest(canonical_schema_json(schema)?.as_bytes());
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, canonicalize(value)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        primitive => primitive,
    }
}

#[cfg(test)]
mod tests {
    use crate::schema::generate_schema_from_json_str;

    use super::*;

    #[test]
    fn object_key_order_does_not_change_canonical_schema_or_hash() {
        let left = generate_schema_from_json_str(
            r#"{"type":"object","properties":{"b":{"type":"int"},"a":{"type":"boolean","true_probability":50}}}"#,
        )
        .unwrap();
        let right = generate_schema_from_json_str(
            r#"{"properties":{"a":{"true_probability":50,"type":"boolean"},"b":{"type":"int"}},"type":"object"}"#,
        )
        .unwrap();

        assert_eq!(
            canonical_schema_json(&left).unwrap(),
            canonical_schema_json(&right).unwrap()
        );
        assert_eq!(
            contract_hash(&left).unwrap(),
            contract_hash(&right).unwrap()
        );
    }
}
