use std::collections::BTreeMap;

use serde_json::Value;
use thiserror::Error;

use crate::project::{BindingCoercion, BindingDefinition};

use super::MockRequest;

#[derive(Debug, Error)]
pub enum BindingError {
    #[error("unsupported binding source {0:?}")]
    UnsupportedSource(String),
    #[error("binding source {0:?} was not present in the request")]
    MissingSource(String),
    #[error("binding target {0:?} does not exist in the generated response")]
    MissingTarget(String),
    #[error("binding source {0:?} could not be coerced to a JSON integer")]
    InvalidInteger(String),
}

pub fn apply_bindings(
    value: &mut Value,
    definitions: &[BindingDefinition],
    request: &MockRequest,
    path_parameters: &BTreeMap<String, String>,
) -> Result<(), BindingError> {
    for definition in definitions {
        let replacement = resolve_source(&definition.source, request, path_parameters)?;
        let replacement = coerce_value(replacement, definition.coerce, &definition.source)?;
        let Some(target) = value.pointer_mut(&definition.target) else {
            return Err(BindingError::MissingTarget(definition.target.clone()));
        };
        *target = replacement;
    }
    Ok(())
}

fn resolve_source(
    expression: &str,
    request: &MockRequest,
    path_parameters: &BTreeMap<String, String>,
) -> Result<Value, BindingError> {
    let source = expression
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
        .ok_or_else(|| BindingError::UnsupportedSource(expression.to_string()))?;

    if let Some(name) = source.strip_prefix("request.path.") {
        return path_parameters
            .get(name)
            .cloned()
            .map(Value::String)
            .ok_or_else(|| BindingError::MissingSource(expression.to_string()));
    }
    if let Some(name) = source.strip_prefix("request.query.") {
        return request
            .query
            .get(name)
            .cloned()
            .map(Value::String)
            .ok_or_else(|| BindingError::MissingSource(expression.to_string()));
    }
    if let Some(name) = source.strip_prefix("request.header.") {
        return request
            .headers
            .get(&name.to_ascii_lowercase())
            .cloned()
            .map(Value::String)
            .ok_or_else(|| BindingError::MissingSource(expression.to_string()));
    }
    if let Some(pointer) = source.strip_prefix("request.body.") {
        return request
            .body
            .as_ref()
            .and_then(|value| value.pointer(pointer))
            .cloned()
            .ok_or_else(|| BindingError::MissingSource(expression.to_string()));
    }
    Err(BindingError::UnsupportedSource(expression.to_string()))
}

fn coerce_value(
    value: Value,
    coercion: Option<BindingCoercion>,
    source: &str,
) -> Result<Value, BindingError> {
    match coercion {
        None => Ok(value),
        Some(BindingCoercion::Integer) => coerce_integer(value, source),
    }
}

fn coerce_integer(value: Value, source: &str) -> Result<Value, BindingError> {
    match value {
        Value::Number(number) if number.is_i64() || number.is_u64() => Ok(Value::Number(number)),
        Value::String(text) => {
            let parsed = (text.trim() == text)
                .then(|| serde_json::from_str::<Value>(&text).ok())
                .flatten()
                .filter(|parsed| {
                    parsed
                        .as_number()
                        .is_some_and(|number| number.is_i64() || number.is_u64())
                });
            parsed.ok_or_else(|| BindingError::InvalidInteger(source.to_string()))
        }
        _ => Err(BindingError::InvalidInteger(source.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn request() -> MockRequest {
        MockRequest {
            method: "GET".into(),
            service: "tasks".into(),
            path: "/tasks/42".into(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: None,
        }
    }

    fn binding(coerce: Option<BindingCoercion>) -> BindingDefinition {
        BindingDefinition {
            target: "/taskId".into(),
            source: "${request.path.taskId}".into(),
            coerce,
        }
    }

    #[test]
    fn path_bindings_remain_strings_without_explicit_coercion() {
        let mut response = json!({"taskId": null});
        let path_parameters = BTreeMap::from([("taskId".into(), "42".into())]);

        apply_bindings(
            &mut response,
            &[binding(None)],
            &request(),
            &path_parameters,
        )
        .unwrap();

        assert_eq!(response["taskId"], json!("42"));
    }

    #[test]
    fn explicitly_coerces_path_bindings_to_integers() {
        for (raw, expected) in [
            ("42", json!(42)),
            ("-9223372036854775808", json!(i64::MIN)),
            ("18446744073709551615", json!(u64::MAX)),
        ] {
            let mut response = json!({"taskId": null});
            let path_parameters = BTreeMap::from([("taskId".into(), raw.into())]);

            apply_bindings(
                &mut response,
                &[binding(Some(BindingCoercion::Integer))],
                &request(),
                &path_parameters,
            )
            .unwrap();

            assert_eq!(response["taskId"], expected, "raw value {raw:?}");
        }
    }

    #[test]
    fn rejects_invalid_or_out_of_range_integer_coercions_without_mutating_the_target() {
        for raw in [
            "",
            "+1",
            "01",
            "1.0",
            "1e3",
            " 42",
            "42 ",
            "18446744073709551616",
            "-9223372036854775809",
        ] {
            let mut response = json!({"taskId": "original"});
            let path_parameters = BTreeMap::from([("taskId".into(), raw.into())]);

            let error = apply_bindings(
                &mut response,
                &[binding(Some(BindingCoercion::Integer))],
                &request(),
                &path_parameters,
            )
            .unwrap_err();

            assert!(
                matches!(error, BindingError::InvalidInteger(_)),
                "raw value {raw:?}: {error}"
            );
            assert_eq!(response["taskId"], "original");
        }
    }

    #[test]
    fn integer_coercion_accepts_typed_body_integers_and_rejects_other_json_types() {
        let source = "${request.body./taskId}";
        assert_eq!(
            coerce_value(json!(42), Some(BindingCoercion::Integer), source).unwrap(),
            json!(42)
        );
        for value in [json!(1.5), json!(true), json!(null), json!({"id": 42})] {
            assert!(matches!(
                coerce_value(value, Some(BindingCoercion::Integer), source),
                Err(BindingError::InvalidInteger(_))
            ));
        }
    }
}
