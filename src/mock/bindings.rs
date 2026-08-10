use std::collections::BTreeMap;

use serde_json::Value;
use thiserror::Error;

use crate::project::BindingDefinition;

use super::MockRequest;

#[derive(Debug, Error)]
pub enum BindingError {
    #[error("unsupported binding source {0:?}")]
    UnsupportedSource(String),
    #[error("binding source {0:?} was not present in the request")]
    MissingSource(String),
    #[error("binding target {0:?} does not exist in the generated response")]
    MissingTarget(String),
}

pub fn apply_bindings(
    value: &mut Value,
    definitions: &[BindingDefinition],
    request: &MockRequest,
    path_parameters: &BTreeMap<String, String>,
) -> Result<(), BindingError> {
    for definition in definitions {
        let replacement = resolve_source(&definition.source, request, path_parameters)?;
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
