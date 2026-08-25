use std::{collections::BTreeMap, fs, path::Path};

use serde_json::{Map, Value, json};

use super::protocol::evidence_requirements;
use super::schema_walk::{
    SubschemaKind, is_non_assertion_ref_sibling, subschema_kind, walk_schema, walk_schema_mut,
};
use super::{
    DiagnosticSeverity, EndpointSelector, FieldEvidence, JSON_SCHEMA_DRAFT_2020_12,
    ProviderDiagnostic, ProviderError, ProviderIdentity, ProviderResponse, Result,
    fingerprint_bytes, validate_provider_response,
};

const JSON_SCHEMA_PROVIDER: &str = "randomizer.json-schema";
const OPENAPI_PROVIDER: &str = "randomizer.openapi";
const SERIALIZED_EXAMPLE_PROVIDER: &str = "randomizer.serialized-example";
const OPENAPI_31_BASE_DIALECT: &str = "https://spec.openapis.org/oas/3.1/dialect/base";

pub fn import_json_schema_file(
    path: impl AsRef<Path>,
    endpoint: EndpointSelector,
    root_symbol: Option<String>,
) -> Result<ProviderResponse> {
    let path = path.as_ref();
    let contents = fs::read(path).map_err(|source| ProviderError::ReadSource {
        path: path.to_path_buf(),
        source,
    })?;
    import_json_schema_bytes(path.to_string_lossy(), &contents, endpoint, root_symbol)
}

pub fn import_json_schema_bytes(
    source_path: impl Into<String>,
    contents: &[u8],
    endpoint: EndpointSelector,
    root_symbol: Option<String>,
) -> Result<ProviderResponse> {
    endpoint.validate()?;
    let source_path = source_path.into();
    let document = parse_source(&source_path, contents)?;
    let mut resolution = ResolutionState::default();
    let schema = resolve_local_references(
        &document,
        &document,
        ResolveContext::Schema,
        "",
        "",
        &mut resolution,
    )?;
    let origins = resolution.origins;
    let edge_origins = resolution.edge_origins;
    require_explicit_draft_2020_12(&schema)?;

    let mut response = ProviderResponse::new(
        ProviderIdentity::new(JSON_SCHEMA_PROVIDER, env!("CARGO_PKG_VERSION")),
        endpoint,
        schema,
        vec![fingerprint_bytes(&source_path, contents)],
    );
    response.root_symbol = root_symbol;
    response.evidence = collect_evidence(
        &response.schema,
        response.root_symbol.is_some(),
        &source_path,
        &origins,
        &edge_origins,
    );
    validate_provider_response(&response)?;
    Ok(response)
}

pub fn import_openapi_response_file(
    path: impl AsRef<Path>,
    endpoint: EndpointSelector,
    root_symbol: Option<String>,
) -> Result<ProviderResponse> {
    let path = path.as_ref();
    let contents = fs::read(path).map_err(|source| ProviderError::ReadSource {
        path: path.to_path_buf(),
        source,
    })?;
    import_openapi_response_bytes(path.to_string_lossy(), &contents, endpoint, root_symbol)
}

pub fn import_serialized_example_file(
    path: impl AsRef<Path>,
    endpoint: EndpointSelector,
    root_symbol: Option<String>,
) -> Result<ProviderResponse> {
    let path = path.as_ref();
    let contents = fs::read(path).map_err(|source| ProviderError::ReadSource {
        path: path.to_path_buf(),
        source,
    })?;
    import_serialized_example_bytes(path.to_string_lossy(), &contents, endpoint, root_symbol)
}

pub fn import_serialized_example_bytes(
    source_path: impl Into<String>,
    contents: &[u8],
    endpoint: EndpointSelector,
    root_symbol: Option<String>,
) -> Result<ProviderResponse> {
    endpoint.validate()?;
    let source_path = source_path.into();
    let example = parse_source(&source_path, contents)?;
    let mut schema = infer_example_schema(&example);
    let object = schema
        .as_object_mut()
        .expect("inferred schemas always have an object root");
    object.insert(
        "$schema".to_string(),
        Value::String(JSON_SCHEMA_DRAFT_2020_12.to_string()),
    );
    object.insert("examples".to_string(), Value::Array(vec![example]));

    let origins = BTreeMap::from([(String::new(), String::new())]);
    let mut response = ProviderResponse::new(
        ProviderIdentity::new(SERIALIZED_EXAMPLE_PROVIDER, env!("CARGO_PKG_VERSION")),
        endpoint,
        schema,
        vec![fingerprint_bytes(&source_path, contents)],
    );
    response.root_symbol = root_symbol;
    response.evidence = collect_evidence(
        &response.schema,
        response.root_symbol.is_some(),
        &source_path,
        &origins,
        &BTreeMap::new(),
    );
    response.diagnostics.push(ProviderDiagnostic {
        severity: DiagnosticSeverity::Warning,
        code: "example_inference_limits".to_string(),
        message: "a serialized example cannot prove enum membership, nullability, or optionality; only observed value shapes and unambiguous formats were inferred"
            .to_string(),
        source_path: Some(source_path),
        source_location: Some("#".to_string()),
    });
    validate_provider_response(&response)?;
    Ok(response)
}

pub fn import_openapi_response_bytes(
    source_path: impl Into<String>,
    contents: &[u8],
    endpoint: EndpointSelector,
    root_symbol: Option<String>,
) -> Result<ProviderResponse> {
    endpoint.validate()?;
    let source_path = source_path.into();
    let document = parse_source(&source_path, contents)?;
    validate_openapi_version(&document)?;

    let method = endpoint.method.to_ascii_lowercase();
    let path_item = document
        .get("paths")
        .and_then(|paths| paths.get(&endpoint.path))
        .ok_or_else(|| ProviderError::OpenApiPathNotFound {
            path: endpoint.path.clone(),
        })?;
    let operation =
        path_item
            .get(&method)
            .ok_or_else(|| ProviderError::OpenApiOperationNotFound {
                method: endpoint.method.clone(),
                path: endpoint.path.clone(),
            })?;
    let status = endpoint.status.to_string();
    let response_document = operation
        .get("responses")
        .and_then(|responses| responses.get(&status))
        .ok_or_else(|| ProviderError::OpenApiResponseNotFound {
            method: endpoint.method.clone(),
            path: endpoint.path.clone(),
            status: endpoint.status,
        })?;
    let response_source_pointer = format!(
        "/paths/{}/{}/responses/{}",
        escape_pointer_token(&endpoint.path),
        escape_pointer_token(&method),
        escape_pointer_token(&status)
    );

    // Resolve the response before selecting `content`: OpenAPI permits a response
    // object itself to be a local component reference.
    let mut response_resolution = ResolutionState::default();
    let resolved_response = resolve_local_references(
        &document,
        response_document,
        ResolveContext::OpenApiResponse,
        &response_source_pointer,
        "",
        &mut response_resolution,
    )?;
    let (media_type, schema_prefix, selected_schema) =
        select_response_schema(&resolved_response, &endpoint)?;
    let schema_source_pointer = response_resolution
        .origins
        .get(&schema_prefix)
        .cloned()
        .unwrap_or_else(|| format!("{response_source_pointer}{schema_prefix}"));
    let mut schema_resolution = ResolutionState::default();
    let mut schema = resolve_local_references(
        &document,
        &selected_schema,
        ResolveContext::Schema,
        &schema_source_pointer,
        "",
        &mut schema_resolution,
    )?;
    let mut origins = schema_resolution.origins;
    let edge_origins = schema_resolution.edge_origins;

    reject_legacy_openapi_nullable(&schema, "")?;
    normalize_openapi_schema_dialects(&mut schema, "")?;
    ensure_draft_2020_12(&mut schema, &mut origins, &response_source_pointer)?;

    let mut selected_endpoint = endpoint;
    selected_endpoint.media_type = Some(media_type);
    let mut response = ProviderResponse::new(
        ProviderIdentity::new(OPENAPI_PROVIDER, env!("CARGO_PKG_VERSION")),
        selected_endpoint,
        schema,
        vec![fingerprint_bytes(&source_path, contents)],
    );
    response.root_symbol = root_symbol;
    response.evidence = collect_evidence(
        &response.schema,
        response.root_symbol.is_some(),
        &source_path,
        &origins,
        &edge_origins,
    );
    validate_provider_response(&response)?;
    Ok(response)
}

fn parse_source(source_path: &str, contents: &[u8]) -> Result<Value> {
    match serde_json::from_slice(contents) {
        Ok(value) => Ok(value),
        Err(json_error) => {
            serde_yaml::from_slice(contents).map_err(|yaml_error| ProviderError::ParseSource {
                path: source_path.to_string(),
                message: format!("not valid JSON ({json_error}) or YAML ({yaml_error})"),
            })
        }
    }
}

fn validate_openapi_version(document: &Value) -> Result<()> {
    let version = document
        .get("openapi")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !version.starts_with("3.1.") {
        return Err(ProviderError::UnsupportedOpenApiVersion {
            version: version.to_string(),
        });
    }
    if let Some(dialect) = document.get("jsonSchemaDialect") {
        let Some(dialect) = dialect.as_str() else {
            return Err(ProviderError::UnsupportedOpenApiDialect {
                dialect: dialect.to_string(),
            });
        };
        let normalized = dialect.trim_end_matches('#');
        if normalized != JSON_SCHEMA_DRAFT_2020_12 && normalized != OPENAPI_31_BASE_DIALECT {
            return Err(ProviderError::UnsupportedOpenApiDialect {
                dialect: dialect.to_string(),
            });
        }
    }
    Ok(())
}

fn require_explicit_draft_2020_12(schema: &Value) -> Result<()> {
    let object = schema.as_object().ok_or_else(|| {
        ProviderError::InvalidSchema(
            "the selected response schema must have an object root".to_string(),
        )
    })?;
    let dialect = object.get("$schema").and_then(Value::as_str);
    if dialect.map(|value| value.trim_end_matches('#')) != Some(JSON_SCHEMA_DRAFT_2020_12) {
        return Err(ProviderError::InvalidSchema(format!(
            "standalone JSON Schema must explicitly declare $schema as {:?}, got {:?}",
            JSON_SCHEMA_DRAFT_2020_12, dialect
        )));
    }
    Ok(())
}

fn infer_example_schema(example: &Value) -> Value {
    match example {
        Value::Null => json!({"type": "null"}),
        Value::Bool(_) => json!({"type": "boolean"}),
        Value::Number(number) if number.is_i64() || number.is_u64() => {
            json!({"type": "integer"})
        }
        Value::Number(_) => json!({"type": "number"}),
        Value::String(value) => {
            let mut schema =
                Map::from_iter([("type".to_string(), Value::String("string".to_string()))]);
            if let Some(format) = unambiguous_string_format(value) {
                schema.insert("format".to_string(), Value::String(format.to_string()));
            }
            Value::Object(schema)
        }
        Value::Array(values) => {
            let mut unique = Vec::new();
            for value in values {
                let inferred = infer_example_schema(value);
                if !unique.contains(&inferred) {
                    unique.push(inferred);
                }
            }
            match unique.as_slice() {
                [] => json!({"type": "array"}),
                [only] => json!({"type": "array", "items": only}),
                _ => json!({"type": "array", "items": {"anyOf": unique}}),
            }
        }
        Value::Object(values) => {
            let properties = values
                .iter()
                .map(|(key, value)| (key.clone(), infer_example_schema(value)))
                .collect::<Map<_, _>>();
            json!({
                "type": "object",
                "properties": properties
            })
        }
    }
}

fn unambiguous_string_format(value: &str) -> Option<&'static str> {
    if is_canonical_uuid(value) {
        return Some("uuid");
    }
    if value.contains('T')
        && has_explicit_timezone(value)
        && validates_string_format(value, "date-time")
    {
        return Some("date-time");
    }
    if value.len() == 10 && validates_string_format(value, "date") {
        return Some("date");
    }
    None
}

fn is_canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && uuid::Uuid::parse_str(value)
            .is_ok_and(|parsed| parsed.hyphenated().to_string().eq_ignore_ascii_case(value))
}

fn has_explicit_timezone(value: &str) -> bool {
    if value.ends_with('Z') || value.ends_with('z') {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 6
        && matches!(bytes[bytes.len() - 6], b'+' | b'-')
        && bytes[bytes.len() - 3] == b':'
        && bytes[bytes.len() - 5..bytes.len() - 3]
            .iter()
            .all(u8::is_ascii_digit)
        && bytes[bytes.len() - 2..].iter().all(u8::is_ascii_digit)
}

fn validates_string_format(value: &str, format: &str) -> bool {
    let schema = json!({"type": "string", "format": format});
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .is_ok_and(|validator| validator.is_valid(&Value::String(value.to_string())))
}

fn select_response_schema(
    response: &Value,
    endpoint: &EndpointSelector,
) -> Result<(String, String, Value)> {
    let content = response
        .get("content")
        .and_then(Value::as_object)
        .ok_or_else(|| ProviderError::OpenApiResponseContentMissing {
            method: endpoint.method.clone(),
            path: endpoint.path.clone(),
            status: endpoint.status,
        })?;
    let available = content.keys().cloned().collect::<Vec<_>>();
    let selected = if let Some(requested) = endpoint.media_type.as_deref() {
        if !content.contains_key(requested) {
            return Err(ProviderError::OpenApiMediaTypeNotFound {
                media_type: requested.to_string(),
                available: available.join(", "),
            });
        }
        requested.to_string()
    } else {
        let schema_bearing = content
            .iter()
            .filter_map(|(media_type, definition)| {
                definition.get("schema").map(|_| media_type.clone())
            })
            .collect::<Vec<_>>();
        match schema_bearing.as_slice() {
            [only] => only.clone(),
            [] => {
                return Err(ProviderError::OpenApiResponseSchemaMissing {
                    media_type: available.join(", "),
                });
            }
            _ => {
                return Err(ProviderError::AmbiguousOpenApiMediaType {
                    available: schema_bearing.join(", "),
                });
            }
        }
    };
    let schema = content[&selected].get("schema").cloned().ok_or_else(|| {
        ProviderError::OpenApiResponseSchemaMissing {
            media_type: selected.clone(),
        }
    })?;
    let schema_prefix = format!("/content/{}/schema", escape_pointer_token(&selected));
    Ok((selected, schema_prefix, schema))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolveContext {
    Schema,
    SchemaArray,
    SchemaMap,
    OpenApiResponse,
    Opaque,
}

impl ResolveContext {
    fn resolves_references(self) -> bool {
        matches!(self, Self::Schema | Self::OpenApiResponse)
    }

    fn object_child(self, keyword: &str) -> Self {
        match self {
            Self::Schema => match subschema_kind(keyword) {
                Some(SubschemaKind::Direct) => Self::Schema,
                Some(SubschemaKind::Array) => Self::SchemaArray,
                Some(SubschemaKind::Map) => Self::SchemaMap,
                None => Self::Opaque,
            },
            Self::SchemaMap => Self::Schema,
            _ => Self::Opaque,
        }
    }

    fn array_child(self) -> Self {
        if self == Self::SchemaArray {
            Self::Schema
        } else {
            Self::Opaque
        }
    }
}

#[derive(Debug, Default)]
struct ResolutionState {
    stack: Vec<String>,
    origins: BTreeMap<String, String>,
    edge_origins: BTreeMap<String, String>,
}

fn resolve_local_references(
    document: &Value,
    value: &Value,
    context: ResolveContext,
    source_pointer: &str,
    output_pointer: &str,
    state: &mut ResolutionState,
) -> Result<Value> {
    state
        .edge_origins
        .entry(output_pointer.to_string())
        .or_insert_with(|| source_pointer.to_string());
    state
        .origins
        .insert(output_pointer.to_string(), source_pointer.to_string());
    match value {
        Value::Object(object) => {
            if context.resolves_references()
                && let Some(reference) = object.get("$ref").and_then(Value::as_str)
            {
                let sibling_keys = object
                    .keys()
                    .filter(|key| key.as_str() != "$ref")
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                let assertion_siblings = sibling_keys
                    .iter()
                    .copied()
                    .filter(|key| !is_non_assertion_ref_sibling(key))
                    .collect::<Vec<_>>();
                if context == ResolveContext::Schema && !assertion_siblings.is_empty() {
                    return Err(ProviderError::InvalidSchema(format!(
                        "schema reference {reference:?} has assertion sibling keywords ({}); pre-dereference or flatten the composition into one supported schema before importing",
                        assertion_siblings.join(", ")
                    )));
                }
                if context == ResolveContext::OpenApiResponse
                    && sibling_keys
                        .iter()
                        .any(|key| *key != "summary" && *key != "description")
                {
                    return Err(ProviderError::InvalidSchema(format!(
                        "OpenAPI response reference {reference:?} has unsupported sibling fields ({}); only summary and description are allowed",
                        sibling_keys.join(", ")
                    )));
                }
                let pointer = local_reference_pointer(reference)?;
                if state.stack.iter().any(|active| active == reference) {
                    return Err(ProviderError::CyclicReference {
                        reference: reference.to_string(),
                    });
                }
                let target = document.pointer(pointer).ok_or_else(|| {
                    ProviderError::UnresolvedReference {
                        reference: reference.to_string(),
                    }
                })?;
                state.stack.push(reference.to_string());
                let mut resolved = resolve_local_references(
                    document,
                    target,
                    context,
                    pointer,
                    output_pointer,
                    state,
                )?;
                state.stack.pop();

                if object.len() > 1 {
                    let resolved_object = resolved.as_object_mut().ok_or_else(|| {
                        ProviderError::InvalidSchema(format!(
                            "reference {reference:?} has sibling keywords but does not resolve to an object"
                        ))
                    })?;
                    for (key, sibling) in object.iter().filter(|(key, _)| key.as_str() != "$ref") {
                        let sibling_source = join_pointer(source_pointer, key);
                        let sibling_output = join_pointer(output_pointer, key);
                        resolved_object.insert(
                            key.clone(),
                            resolve_local_references(
                                document,
                                sibling,
                                context.object_child(key),
                                &sibling_source,
                                &sibling_output,
                                state,
                            )?,
                        );
                    }
                }
                Ok(resolved)
            } else {
                let mut resolved = Map::new();
                for (key, nested) in object {
                    let nested_source = join_pointer(source_pointer, key);
                    let nested_output = join_pointer(output_pointer, key);
                    resolved.insert(
                        key.clone(),
                        resolve_local_references(
                            document,
                            nested,
                            context.object_child(key),
                            &nested_source,
                            &nested_output,
                            state,
                        )?,
                    );
                }
                Ok(Value::Object(resolved))
            }
        }
        Value::Array(values) => Ok(Value::Array(
            values
                .iter()
                .enumerate()
                .map(|(index, nested)| {
                    resolve_local_references(
                        document,
                        nested,
                        context.array_child(),
                        &join_pointer(source_pointer, &index.to_string()),
                        &join_pointer(output_pointer, &index.to_string()),
                        state,
                    )
                })
                .collect::<Result<Vec<_>>>()?,
        )),
        _ => Ok(value.clone()),
    }
}

fn local_reference_pointer(reference: &str) -> Result<&str> {
    if !reference.starts_with('#') {
        return Err(ProviderError::ExternalReference {
            reference: reference.to_string(),
        });
    }
    let pointer = &reference[1..];
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return Err(ProviderError::InvalidLocalReference {
            reference: reference.to_string(),
        });
    }
    Ok(pointer)
}

fn ensure_draft_2020_12(
    schema: &mut Value,
    origins: &mut BTreeMap<String, String>,
    fallback_origin: &str,
) -> Result<()> {
    let object = schema.as_object_mut().ok_or_else(|| {
        ProviderError::InvalidSchema(
            "the selected response schema must have an object root".to_string(),
        )
    })?;
    if let Some(dialect) = object.get("$schema").and_then(Value::as_str) {
        let normalized = dialect.trim_end_matches('#');
        if normalized != JSON_SCHEMA_DRAFT_2020_12 && normalized != OPENAPI_31_BASE_DIALECT {
            return Err(ProviderError::InvalidSchema(format!(
                "unsupported schema dialect {dialect:?}; expected Draft 2020-12 or the OpenAPI 3.1 base dialect"
            )));
        }
    }
    if object.get("$schema").and_then(Value::as_str) != Some(JSON_SCHEMA_DRAFT_2020_12) {
        object.insert(
            "$schema".to_string(),
            Value::String(JSON_SCHEMA_DRAFT_2020_12.to_string()),
        );
        let origin = origins
            .get("")
            .cloned()
            .unwrap_or_else(|| fallback_origin.to_string());
        origins.insert("/$schema".to_string(), origin);
    }
    Ok(())
}

fn normalize_openapi_schema_dialects(value: &mut Value, pointer: &str) -> Result<()> {
    walk_schema_mut(value, pointer, &mut |schema, schema_pointer| {
        let Some(dialect) = schema.get_mut("$schema") else {
            return Ok(());
        };
        let Some(declared) = dialect.as_str() else {
            return Err(ProviderError::InvalidSchema(format!(
                "$schema at #{} must be a string",
                if schema_pointer.is_empty() {
                    "/"
                } else {
                    schema_pointer
                }
            )));
        };
        let normalized = declared.trim_end_matches('#');
        if normalized != JSON_SCHEMA_DRAFT_2020_12 && normalized != OPENAPI_31_BASE_DIALECT {
            return Err(ProviderError::InvalidSchema(format!(
                "unsupported schema dialect {declared:?} at #{}; expected Draft 2020-12 or the OpenAPI 3.1 base dialect",
                if schema_pointer.is_empty() {
                    "/"
                } else {
                    schema_pointer
                }
            )));
        }
        *dialect = Value::String(JSON_SCHEMA_DRAFT_2020_12.to_string());
        Ok(())
    })
}

fn reject_legacy_openapi_nullable(value: &Value, pointer: &str) -> Result<()> {
    walk_schema(value, pointer, &mut |schema, schema_pointer| {
        if schema.get("nullable").is_some() {
            return Err(ProviderError::InvalidSchema(format!(
                "OpenAPI 3.1 response schema uses legacy keyword \"nullable\" at #{}; use a JSON Schema null union",
                if schema_pointer.is_empty() {
                    "/"
                } else {
                    schema_pointer
                }
            )));
        }
        Ok(())
    })
}

fn collect_evidence(
    schema: &Value,
    has_root_symbol: bool,
    source_path: &str,
    origins: &BTreeMap<String, String>,
    edge_origins: &BTreeMap<String, String>,
) -> Vec<FieldEvidence> {
    evidence_requirements(schema, has_root_symbol)
        .into_iter()
        .map(|requirement| {
            evidence_for(
                requirement
                    .schema_path
                    .strip_prefix('#')
                    .expect("evidence requirements always use fragment paths"),
                requirement.kind,
                source_path,
                origins,
                edge_origins,
            )
        })
        .collect()
}

fn evidence_for(
    pointer: &str,
    kind: &str,
    source_path: &str,
    origins: &BTreeMap<String, String>,
    edge_origins: &BTreeMap<String, String>,
) -> FieldEvidence {
    let source_location = if matches!(
        kind,
        "response_wrapper" | "root_symbol" | "property_name" | "requiredness"
    ) {
        edge_origins
            .get(pointer)
            .map(String::as_str)
            .unwrap_or_else(|| closest_origin(pointer, origins))
    } else {
        closest_origin(pointer, origins)
    };
    FieldEvidence {
        schema_path: format!("#{pointer}"),
        source_path: source_path.to_string(),
        source_location: format!("#{source_location}"),
        kind: kind.to_string(),
    }
}

fn closest_origin<'a>(pointer: &str, origins: &'a BTreeMap<String, String>) -> &'a str {
    let mut candidate = pointer;
    loop {
        if let Some(origin) = origins.get(candidate) {
            return origin;
        }
        let Some(index) = candidate.rfind('/') else {
            return origins.get("").map(String::as_str).unwrap_or("");
        };
        candidate = &candidate[..index];
    }
}

fn join_pointer(base: &str, token: &str) -> String {
    format!("{base}/{}", escape_pointer_token(token))
}

fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn standalone_schema_resolves_local_refs_and_records_field_evidence() {
        let source = serde_json::to_vec(&json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "$defs": {
                "state": {"type": "string", "enum": ["ACTIVE", "PAUSED"]}
            },
            "type": "object",
            "properties": {
                "state": {"$ref": "#/$defs/state"},
                "created_at": {"type": "string", "format": "date-time"}
            },
            "required": ["state", "created_at"]
        }))
        .unwrap();
        let imported = import_json_schema_bytes(
            "contracts/user.json",
            &source,
            EndpointSelector::new("GET", "/users/{id}", 200),
            Some("UserResponse".to_string()),
        )
        .unwrap();

        assert_eq!(imported.schema["$schema"], JSON_SCHEMA_DRAFT_2020_12);
        assert_eq!(
            imported.schema["properties"]["state"]["enum"],
            json!(["ACTIVE", "PAUSED"])
        );
        assert_eq!(
            imported.schema["properties"]["created_at"]["format"],
            "date-time"
        );
        assert!(imported.schema["properties"]["state"].get("$ref").is_none());
        assert!(imported.evidence.iter().any(|evidence| {
            evidence.schema_path == "#/properties/state"
                && evidence.kind == "property_name"
                && evidence.source_location == "#/properties/state"
        }));
        assert!(imported.evidence.iter().any(|evidence| {
            evidence.schema_path == "#/properties/state/enum"
                && evidence.kind == "enum"
                && evidence.source_location == "#/$defs/state/enum"
        }));
        assert!(
            imported
                .evidence
                .iter()
                .any(|evidence| evidence.schema_path == "#" && evidence.kind == "root_symbol")
        );
    }

    #[test]
    fn standalone_schema_requires_an_explicit_2020_12_dialect() {
        let error = import_json_schema_bytes(
            "contracts/ambiguous.json",
            br#"{"type":"object"}"#,
            EndpointSelector::new("GET", "/users", 200),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("must explicitly declare $schema"), "{error}");
    }

    #[test]
    fn standalone_schema_rejects_ref_siblings_instead_of_overwriting_constraints() {
        let source = serde_json::to_vec(&json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "$defs": {"identifier": {"type": "string"}},
            "$ref": "#/$defs/identifier",
            "type": "integer"
        }))
        .unwrap();

        let error = import_json_schema_bytes(
            "contracts/invalid.json",
            &source,
            EndpointSelector::new("GET", "/items", 200),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("assertion sibling keywords"), "{error}");
        assert!(error.contains("pre-dereference or flatten"), "{error}");
    }

    #[test]
    fn standalone_root_ref_accepts_dialect_and_definition_metadata() {
        let source = serde_json::to_vec(&json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "$defs": {"identifier": {"type": "string"}},
            "$ref": "#/$defs/identifier"
        }))
        .unwrap();

        let imported = import_json_schema_bytes(
            "contracts/identifier.json",
            &source,
            EndpointSelector::new("GET", "/identifier", 200),
            None,
        )
        .unwrap();

        assert_eq!(imported.schema["type"], "string");
        assert_eq!(imported.schema["$schema"], JSON_SCHEMA_DRAFT_2020_12);
    }

    #[test]
    fn openapi_31_import_is_self_contained_and_preserves_null_unions() {
        let source = br#"
openapi: 3.1.0
jsonSchemaDialect: https://spec.openapis.org/oas/3.1/dialect/base
info: {title: Users, version: '1'}
paths:
  /users/{id}:
    get:
      responses:
        '200':
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/User'
components:
  schemas:
    User:
      type: object
      required: [state, created_at]
      properties:
        state:
          $ref: '#/components/schemas/State'
        created_at:
          type: string
          format: date-time
        nickname:
          type: [string, 'null']
    State:
      type: string
      enum: [ACTIVE, PAUSED]
"#;
        let imported = import_openapi_response_bytes(
            "openapi.yaml",
            source,
            EndpointSelector::new("GET", "/users/{id}", 200),
            Some("User".to_string()),
        )
        .unwrap();

        assert_eq!(
            imported.schema["properties"]["state"]["enum"],
            json!(["ACTIVE", "PAUSED"])
        );
        assert_eq!(
            imported.schema["properties"]["created_at"]["format"],
            "date-time"
        );
        assert_eq!(
            imported.schema["properties"]["nickname"],
            json!({"type": ["string", "null"]})
        );
        assert_eq!(
            imported.endpoint.media_type.as_deref(),
            Some("application/json")
        );
        assert!(!imported.schema.to_string().contains("$ref"));
        assert!(imported.diagnostics.is_empty());
        assert!(imported.evidence.iter().any(|evidence| {
            evidence.schema_path == "#/properties/state"
                && evidence.kind == "property_name"
                && evidence.source_location == "#/components/schemas/User/properties/state"
        }));
        assert!(imported.evidence.iter().any(|evidence| {
            evidence.schema_path == "#/properties/nickname/type/1" && evidence.kind == "nullable"
        }));
    }

    #[test]
    fn openapi_import_preserves_schema_like_fields_inside_instance_data() {
        let source = br#"
openapi: 3.1.0
info: {title: Payloads, version: '1'}
paths:
  /payload:
    get:
      responses:
        '200':
          content:
            application/json:
              schema:
                type: object
                examples:
                  - {$schema: domain-value, $ref: 'https://example.test/value', nullable: true}
                properties:
                  payload:
                    const: {$schema: domain-value, $ref: 'https://example.test/value', nullable: true}
"#;
        let imported = import_openapi_response_bytes(
            "openapi.yaml",
            source,
            EndpointSelector::new("GET", "/payload", 200),
            None,
        )
        .unwrap();
        let expected = json!({
            "$schema": "domain-value",
            "$ref": "https://example.test/value",
            "nullable": true
        });

        assert_eq!(imported.schema["examples"][0], expected);
        assert_eq!(imported.schema["properties"]["payload"]["const"], expected);
    }

    #[test]
    fn openapi_import_rejects_30_and_legacy_nullable_keywords() {
        let openapi_30 = br#"
openapi: 3.0.3
info: {title: Users, version: '1'}
paths: {}
"#;
        let error = import_openapi_response_bytes(
            "openapi.yaml",
            openapi_30,
            EndpointSelector::new("GET", "/users/{id}", 200),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("expected OpenAPI 3.1.x"));

        let legacy_nullable = br#"
openapi: 3.1.0
info: {title: Users, version: '1'}
paths:
  /users/{id}:
    get:
      responses:
        '200':
          content:
            application/json:
              schema:
                type: object
                properties:
                  nickname: {type: string, nullable: true}
"#;
        let error = import_openapi_response_bytes(
            "openapi.yaml",
            legacy_nullable,
            EndpointSelector::new("GET", "/users/{id}", 200),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("legacy keyword \"nullable\""));

        let custom_dialect = br#"
openapi: 3.1.0
jsonSchemaDialect: https://example.com/custom
info: {title: Users, version: '1'}
paths: {}
"#;
        let error = import_openapi_response_bytes(
            "openapi.yaml",
            custom_dialect,
            EndpointSelector::new("GET", "/users/{id}", 200),
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("jsonSchemaDialect"), "{error}");
        assert!(error.contains("OpenAPI 3.1 base dialect"), "{error}");
    }

    #[test]
    fn openapi_import_rejects_external_refs_and_ambiguous_content() {
        let external = br#"
openapi: 3.1.0
info: {title: Test, version: '1'}
paths:
  /items:
    get:
      responses:
        '200':
          content:
            application/json:
              schema: {$ref: 'https://example.test/schema.json'}
"#;
        let error = import_openapi_response_bytes(
            "openapi.yaml",
            external,
            EndpointSelector::new("GET", "/items", 200),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("external reference"));

        let ambiguous = br#"
openapi: 3.1.0
info: {title: Test, version: '1'}
paths:
  /items:
    get:
      responses:
        '200':
          content:
            application/json:
              schema: {type: object}
            application/problem+json:
              schema: {type: object}
"#;
        let error = import_openapi_response_bytes(
            "openapi.yaml",
            ambiguous,
            EndpointSelector::new("GET", "/items", 200),
            None,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("multiple schema-bearing media types")
        );
    }

    #[test]
    fn openapi_import_resolves_only_the_selected_media_type_schema() {
        let source = br#"
openapi: 3.1.0
info: {title: Test, version: '1'}
paths:
  /items:
    get:
      responses:
        '200':
          content:
            application/json:
              schema: {type: object}
            application/problem+json:
              schema: {$ref: 'https://example.test/problem.json'}
"#;

        let imported = import_openapi_response_bytes(
            "openapi.yaml",
            source,
            EndpointSelector::new("GET", "/items", 200).with_media_type("application/json"),
            None,
        )
        .unwrap();

        assert_eq!(imported.schema["type"], "object");
        assert_eq!(
            imported.endpoint.media_type.as_deref(),
            Some("application/json")
        );
    }

    #[test]
    fn serialized_examples_infer_only_observed_shapes_and_formats() {
        let source = br#"{
            "id": "7d444840-9dc0-11d1-b245-5ffdce74fad2",
            "created_at": "2024-01-01T00:00:00Z",
            "birthday": "2000-02-29",
            "status": "ACTIVE",
            "values": [1, "two"]
        }"#;
        let imported = import_serialized_example_bytes(
            "fixtures/user.json",
            source,
            EndpointSelector::new("GET", "/users/{id}", 200),
            None,
        )
        .unwrap();

        assert_eq!(imported.schema["properties"]["id"]["format"], "uuid");
        assert_eq!(
            imported.schema["properties"]["created_at"]["format"],
            "date-time"
        );
        assert_eq!(imported.schema["properties"]["birthday"]["format"], "date");
        assert!(
            imported.schema["properties"]["status"]
                .get("enum")
                .is_none()
        );
        assert!(imported.schema.get("required").is_none());
        assert_eq!(
            imported.schema["properties"]["values"]["items"]["anyOf"],
            json!([{"type": "integer"}, {"type": "string"}])
        );
        assert_eq!(imported.schema["examples"].as_array().unwrap().len(), 1);
        assert_eq!(imported.diagnostics[0].code, "example_inference_limits");
    }
}
