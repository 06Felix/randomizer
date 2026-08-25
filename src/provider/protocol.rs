use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    JSON_SCHEMA_DRAFT_2020_12, PROVIDER_PROTOCOL_VERSION, ProviderError, Result, SHA256_ALGORITHM,
    schema_walk::{is_non_assertion_ref_sibling, walk_schema},
};

pub const EVIDENCE_KIND_RESPONSE_WRAPPER: &str = "response_wrapper";
pub const EVIDENCE_KIND_ROOT_SYMBOL: &str = "root_symbol";
pub const EVIDENCE_KIND_PROPERTY_NAME: &str = "property_name";
pub const EVIDENCE_KIND_TYPE: &str = "type";
pub const EVIDENCE_KIND_ENUM: &str = "enum";
pub const EVIDENCE_KIND_CONST: &str = "const";
pub const EVIDENCE_KIND_FORMAT: &str = "format";
pub const EVIDENCE_KIND_REQUIREDNESS: &str = "requiredness";
pub const EVIDENCE_KIND_NULLABLE: &str = "nullable";
pub const EVIDENCE_KIND_CONSTRAINT: &str = "constraint";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EndpointSelector {
    pub method: String,
    pub path: String,
    pub status: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

impl EndpointSelector {
    pub fn new(method: impl Into<String>, path: impl Into<String>, status: u16) -> Self {
        Self {
            method: method.into().to_ascii_uppercase(),
            path: path.into(),
            status,
            media_type: None,
        }
    }

    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.method.is_empty()
            || !self
                .method
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte == b'-')
        {
            return Err(ProviderError::InvalidEndpoint(format!(
                "method {:?} must contain uppercase ASCII letters or '-'",
                self.method
            )));
        }
        if !self.path.starts_with('/') {
            return Err(ProviderError::InvalidEndpoint(format!(
                "path {:?} must start with '/'",
                self.path
            )));
        }
        if self.path.contains('?')
            || self.path.contains('#')
            || self.path.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(ProviderError::InvalidEndpoint(format!(
                "path {:?} must contain only the HTTP path; put query matching in the route and omit fragments/control characters",
                self.path
            )));
        }
        if !(100..=599).contains(&self.status) {
            return Err(ProviderError::InvalidEndpoint(format!(
                "status {} must be between 100 and 599",
                self.status
            )));
        }
        if self
            .media_type
            .as_ref()
            .is_some_and(|media_type| media_type.trim().is_empty())
        {
            return Err(ProviderError::InvalidEndpoint(
                "media_type must not be empty when provided".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequest {
    pub protocol_version: String,
    pub endpoint: EndpointSelector,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_symbol: Option<String>,
    #[serde(default)]
    pub source_paths: Vec<String>,
}

impl ProviderRequest {
    pub fn new(endpoint: EndpointSelector) -> Self {
        Self {
            protocol_version: PROVIDER_PROTOCOL_VERSION.to_string(),
            endpoint,
            root_symbol: None,
            source_paths: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.protocol_version != PROVIDER_PROTOCOL_VERSION {
            return Err(ProviderError::InvalidResponse(format!(
                "unsupported request protocol_version {:?}; expected {:?}",
                self.protocol_version, PROVIDER_PROTOCOL_VERSION
            )));
        }
        self.endpoint.validate()?;
        validate_optional_non_empty("root_symbol", self.root_symbol.as_deref())?;
        if self.source_paths.iter().any(|path| path.trim().is_empty()) {
            return Err(ProviderError::InvalidResponse(
                "source_paths must not contain empty paths".to_string(),
            ));
        }
        let mut paths = BTreeSet::new();
        if let Some(duplicate) = self
            .source_paths
            .iter()
            .find(|path| !paths.insert(path.as_str()))
        {
            return Err(ProviderError::InvalidResponse(format!(
                "source_paths contains duplicate path {duplicate:?}"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderIdentity {
    pub name: String,
    pub version: String,
}

impl ProviderIdentity {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceFingerprint {
    pub path: String,
    pub algorithm: String,
    pub digest: String,
}

pub fn fingerprint_bytes(path: impl Into<String>, contents: &[u8]) -> SourceFingerprint {
    SourceFingerprint {
        path: path.into(),
        algorithm: SHA256_ALGORITHM.to_string(),
        digest: format!("{:x}", Sha256::digest(contents)),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FieldEvidence {
    /// JSON Pointer into `ProviderResponse.schema`, prefixed with `#`.
    pub schema_path: String,
    pub source_path: String,
    /// Provider-specific source location, such as a JSON Pointer or `Type.java:42`.
    pub source_location: String,
    /// A claim kind from the `EVIDENCE_KIND_*` constants. Provider-specific kinds are allowed,
    /// but do not satisfy the core schema claims validated by Randomizer.
    pub kind: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProviderResponse {
    pub protocol_version: String,
    pub provider: ProviderIdentity,
    pub endpoint: EndpointSelector,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_symbol: Option<String>,
    pub schema: Value,
    pub source_fingerprints: Vec<SourceFingerprint>,
    #[serde(default)]
    pub evidence: Vec<FieldEvidence>,
    #[serde(default)]
    pub diagnostics: Vec<ProviderDiagnostic>,
}

impl ProviderResponse {
    pub fn new(
        provider: ProviderIdentity,
        endpoint: EndpointSelector,
        schema: Value,
        source_fingerprints: Vec<SourceFingerprint>,
    ) -> Self {
        Self {
            protocol_version: PROVIDER_PROTOCOL_VERSION.to_string(),
            provider,
            endpoint,
            root_symbol: None,
            schema,
            source_fingerprints,
            evidence: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

pub fn validate_provider_response(response: &ProviderResponse) -> Result<()> {
    if response.protocol_version != PROVIDER_PROTOCOL_VERSION {
        return Err(ProviderError::InvalidResponse(format!(
            "unsupported protocol_version {:?}; expected {:?}",
            response.protocol_version, PROVIDER_PROTOCOL_VERSION
        )));
    }
    response.endpoint.validate()?;
    validate_non_empty("provider.name", &response.provider.name)?;
    validate_non_empty("provider.version", &response.provider.version)?;
    validate_optional_non_empty("root_symbol", response.root_symbol.as_deref())?;

    if response.source_fingerprints.is_empty() {
        return Err(ProviderError::InvalidResponse(
            "source_fingerprints must contain at least one source".to_string(),
        ));
    }
    let mut fingerprint_paths = BTreeSet::new();
    for fingerprint in &response.source_fingerprints {
        validate_non_empty("source_fingerprints.path", &fingerprint.path)?;
        if fingerprint.algorithm != SHA256_ALGORITHM {
            return Err(ProviderError::InvalidResponse(format!(
                "unsupported fingerprint algorithm {:?}; expected {:?}",
                fingerprint.algorithm, SHA256_ALGORITHM
            )));
        }
        if fingerprint.digest.len() != 64
            || !fingerprint
                .digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProviderError::InvalidResponse(format!(
                "source fingerprint for {:?} is not a lowercase SHA-256 digest",
                fingerprint.path
            )));
        }
        if !fingerprint_paths.insert(fingerprint.path.as_str()) {
            return Err(ProviderError::InvalidResponse(format!(
                "duplicate source fingerprint for {:?}",
                fingerprint.path
            )));
        }
    }

    validate_schema(&response.schema)?;

    for evidence in &response.evidence {
        validate_non_empty("evidence.source_path", &evidence.source_path)?;
        validate_non_empty("evidence.source_location", &evidence.source_location)?;
        validate_non_empty("evidence.kind", &evidence.kind)?;
        if !fingerprint_paths.contains(evidence.source_path.as_str()) {
            return Err(ProviderError::InvalidResponse(format!(
                "evidence source {:?} has no source fingerprint",
                evidence.source_path
            )));
        }
        let pointer = evidence.schema_path.strip_prefix('#').ok_or_else(|| {
            ProviderError::InvalidResponse(format!(
                "evidence schema_path {:?} must start with '#'",
                evidence.schema_path
            ))
        })?;
        if pointer.is_empty() {
            // The document root is a valid evidence target.
        } else if !pointer.starts_with('/') || response.schema.pointer(pointer).is_none() {
            return Err(ProviderError::InvalidResponse(format!(
                "evidence schema_path {:?} does not resolve in the response schema",
                evidence.schema_path
            )));
        }
    }
    validate_evidence_coverage(
        &response.schema,
        response.root_symbol.is_some(),
        &response.evidence,
    )?;

    for diagnostic in &response.diagnostics {
        validate_non_empty("diagnostics.code", &diagnostic.code)?;
        validate_non_empty("diagnostics.message", &diagnostic.message)?;
        validate_optional_non_empty("diagnostics.source_path", diagnostic.source_path.as_deref())?;
        validate_optional_non_empty(
            "diagnostics.source_location",
            diagnostic.source_location.as_deref(),
        )?;
        if let Some(source_path) = diagnostic.source_path.as_deref()
            && !fingerprint_paths.contains(source_path)
        {
            return Err(ProviderError::InvalidResponse(format!(
                "diagnostic source {source_path:?} has no source fingerprint"
            )));
        }
    }
    if response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(ProviderError::InvalidResponse(
            "provider returned one or more error diagnostics".to_string(),
        ));
    }

    Ok(())
}

fn validate_evidence_coverage(
    schema: &Value,
    has_root_symbol: bool,
    evidence: &[FieldEvidence],
) -> Result<()> {
    if evidence.is_empty() {
        return Err(ProviderError::InvalidResponse(
            "evidence must identify the source of the response schema".to_string(),
        ));
    }
    for requirement in evidence_requirements(schema, has_root_symbol) {
        let evidenced = evidence.iter().any(|item| {
            item.schema_path == requirement.schema_path && item.kind == requirement.kind
        });
        if !evidenced {
            return Err(ProviderError::InvalidResponse(format!(
                "schema claim {:?} requires exact {:?} evidence",
                requirement.schema_path, requirement.kind
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EvidenceRequirement {
    pub schema_path: String,
    pub kind: &'static str,
}

pub(super) fn evidence_requirements(
    schema: &Value,
    has_root_symbol: bool,
) -> Vec<EvidenceRequirement> {
    let mut requirements = vec![EvidenceRequirement {
        schema_path: "#".to_string(),
        kind: EVIDENCE_KIND_RESPONSE_WRAPPER,
    }];
    if has_root_symbol {
        requirements.push(EvidenceRequirement {
            schema_path: "#".to_string(),
            kind: EVIDENCE_KIND_ROOT_SYMBOL,
        });
    }
    collect_schema_claims(schema, "", &mut requirements);
    requirements
}

fn collect_schema_claims(
    value: &Value,
    pointer: &str,
    requirements: &mut Vec<EvidenceRequirement>,
) {
    let Some(object) = value.as_object() else {
        return;
    };

    for (keyword, kind) in [
        ("type", EVIDENCE_KIND_TYPE),
        ("enum", EVIDENCE_KIND_ENUM),
        ("const", EVIDENCE_KIND_CONST),
        ("format", EVIDENCE_KIND_FORMAT),
    ] {
        if object.contains_key(keyword) {
            requirements.push(EvidenceRequirement {
                schema_path: schema_path(&join_pointer(pointer, keyword)),
                kind,
            });
        }
    }

    for keyword in [
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "minLength",
        "maxLength",
        "pattern",
        "minItems",
        "maxItems",
        "uniqueItems",
        "minContains",
        "maxContains",
        "minProperties",
        "maxProperties",
        "additionalProperties",
        "unevaluatedProperties",
        "unevaluatedItems",
    ] {
        if object.contains_key(keyword) {
            requirements.push(EvidenceRequirement {
                schema_path: schema_path(&join_pointer(pointer, keyword)),
                kind: EVIDENCE_KIND_CONSTRAINT,
            });
        }
    }

    match object.get("type") {
        Some(Value::String(schema_type)) if schema_type == "null" => {
            requirements.push(EvidenceRequirement {
                schema_path: schema_path(&join_pointer(pointer, "type")),
                kind: EVIDENCE_KIND_NULLABLE,
            });
        }
        Some(Value::Array(types)) => {
            for (index, schema_type) in types.iter().enumerate() {
                if schema_type == "null" {
                    requirements.push(EvidenceRequirement {
                        schema_path: schema_path(&join_pointer(
                            &join_pointer(pointer, "type"),
                            &index.to_string(),
                        )),
                        kind: EVIDENCE_KIND_NULLABLE,
                    });
                }
            }
        }
        _ => {}
    }
    if object.get("const") == Some(&Value::Null) {
        requirements.push(EvidenceRequirement {
            schema_path: schema_path(&join_pointer(pointer, "const")),
            kind: EVIDENCE_KIND_NULLABLE,
        });
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        for (index, value) in values.iter().enumerate() {
            if value.is_null() {
                requirements.push(EvidenceRequirement {
                    schema_path: schema_path(&join_pointer(
                        &join_pointer(pointer, "enum"),
                        &index.to_string(),
                    )),
                    kind: EVIDENCE_KIND_NULLABLE,
                });
            }
        }
    }

    if let Some(required) = object.get("required").and_then(Value::as_array) {
        for index in 0..required.len() {
            requirements.push(EvidenceRequirement {
                schema_path: schema_path(&join_pointer(
                    &join_pointer(pointer, "required"),
                    &index.to_string(),
                )),
                kind: EVIDENCE_KIND_REQUIREDNESS,
            });
        }
    }

    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (name, property) in properties {
            let property_pointer = join_pointer(&join_pointer(pointer, "properties"), name);
            requirements.push(EvidenceRequirement {
                schema_path: schema_path(&property_pointer),
                kind: EVIDENCE_KIND_PROPERTY_NAME,
            });
            let is_required = object
                .get("required")
                .and_then(Value::as_array)
                .is_some_and(|required| {
                    required
                        .iter()
                        .any(|required_name| required_name.as_str() == Some(name))
                });
            if !is_required {
                // Optionality is a wire-contract claim even though JSON Schema represents it by
                // absence from the parent's `required` array. Target the property schema itself.
                requirements.push(EvidenceRequirement {
                    schema_path: schema_path(&property_pointer),
                    kind: EVIDENCE_KIND_REQUIREDNESS,
                });
            }
            collect_schema_claims(property, &property_pointer, requirements);
        }
    }

    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = object.get(keyword).and_then(Value::as_array) {
            for (index, branch) in branches.iter().enumerate() {
                let branch_pointer =
                    join_pointer(&join_pointer(pointer, keyword), &index.to_string());
                collect_schema_claims(branch, &branch_pointer, requirements);
            }
        }
    }
    if let Some(items) = object.get("items") {
        collect_schema_claims(items, &join_pointer(pointer, "items"), requirements);
    }
    if let Some(prefix_items) = object.get("prefixItems").and_then(Value::as_array) {
        for (index, item) in prefix_items.iter().enumerate() {
            collect_schema_claims(
                item,
                &join_pointer(&join_pointer(pointer, "prefixItems"), &index.to_string()),
                requirements,
            );
        }
    }
    for keyword in ["contains", "not", "if", "then", "else"] {
        if let Some(nested) = object.get(keyword) {
            collect_schema_claims(nested, &join_pointer(pointer, keyword), requirements);
        }
    }
    for keyword in ["$defs", "patternProperties", "dependentSchemas"] {
        if let Some(schemas) = object.get(keyword).and_then(Value::as_object) {
            for (name, nested) in schemas {
                collect_schema_claims(
                    nested,
                    &join_pointer(&join_pointer(pointer, keyword), name),
                    requirements,
                );
            }
        }
    }
    if let Some(additional_properties) = object.get("additionalProperties")
        && additional_properties.is_object()
    {
        collect_schema_claims(
            additional_properties,
            &join_pointer(pointer, "additionalProperties"),
            requirements,
        );
    }
}

fn schema_path(pointer: &str) -> String {
    format!("#{pointer}")
}

fn join_pointer(base: &str, token: &str) -> String {
    format!("{base}/{}", token.replace('~', "~0").replace('/', "~1"))
}

fn validate_schema(schema: &Value) -> Result<()> {
    let object = schema.as_object().ok_or_else(|| {
        ProviderError::InvalidSchema(
            "the root must be an object so it can declare $schema".to_string(),
        )
    })?;
    let dialect = object.get("$schema").and_then(Value::as_str);
    if dialect.map(|value| value.trim_end_matches('#')) != Some(JSON_SCHEMA_DRAFT_2020_12) {
        return Err(ProviderError::InvalidSchema(format!(
            "$schema must be {:?}, got {:?}",
            JSON_SCHEMA_DRAFT_2020_12, dialect
        )));
    }
    validate_declared_dialects(schema, "")?;
    reject_ref_assertion_siblings(schema)?;
    reject_external_references(schema)?;
    jsonschema::draft202012::meta::validate(schema)
        .map_err(|error| ProviderError::InvalidSchema(error.to_string()))?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|error| ProviderError::InvalidSchema(error.to_string()))?;
    Ok(())
}

fn reject_ref_assertion_siblings(value: &Value) -> Result<()> {
    walk_schema(value, "", &mut |schema, pointer| {
        let Some(object) = schema.as_object() else {
            return Ok(());
        };
        let Some(reference) = object.get("$ref").and_then(Value::as_str) else {
            return Ok(());
        };
        let assertion_siblings = object
            .keys()
            .map(String::as_str)
            .filter(|keyword| keyword != &"$ref" && !is_non_assertion_ref_sibling(keyword))
            .collect::<Vec<_>>();
        if assertion_siblings.is_empty() {
            return Ok(());
        }
        Err(ProviderError::InvalidSchema(format!(
            "schema reference {reference:?} at #{} has assertion sibling keywords ({}); pre-dereference or flatten the composition into one supported schema before importing",
            if pointer.is_empty() { "/" } else { pointer },
            assertion_siblings.join(", ")
        )))
    })
}

fn validate_declared_dialects(value: &Value, pointer: &str) -> Result<()> {
    walk_schema(value, pointer, &mut |schema, schema_pointer| {
        let Some(dialect) = schema.get("$schema") else {
            return Ok(());
        };
        let supported = dialect
            .as_str()
            .is_some_and(|dialect| dialect.trim_end_matches('#') == JSON_SCHEMA_DRAFT_2020_12);
        if supported {
            return Ok(());
        }
        Err(ProviderError::InvalidSchema(format!(
            "schema resource at #{} declares unsupported $schema {dialect}; expected Draft 2020-12",
            if schema_pointer.is_empty() {
                "/"
            } else {
                schema_pointer
            }
        )))
    })
}

fn reject_external_references(value: &Value) -> Result<()> {
    walk_schema(value, "", &mut |schema, _| {
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
            && !reference.starts_with('#')
        {
            return Err(ProviderError::ExternalReference {
                reference: reference.to_string(),
            });
        }
        Ok(())
    })
}

fn validate_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(ProviderError::InvalidResponse(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn validate_optional_non_empty(field: &str, value: Option<&str>) -> Result<()> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        return Err(ProviderError::InvalidResponse(format!(
            "{field} must not be empty when provided"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn add_required_evidence(response: &mut ProviderResponse) {
        response.evidence = evidence_requirements(&response.schema, response.root_symbol.is_some())
            .into_iter()
            .map(|requirement| FieldEvidence {
                schema_path: requirement.schema_path,
                source_path: "schema.json".to_string(),
                source_location: "#".to_string(),
                kind: requirement.kind.to_string(),
            })
            .collect();
    }

    #[test]
    fn fingerprints_are_stable_sha256_values() {
        assert_eq!(
            fingerprint_bytes("schema.json", b"abc"),
            SourceFingerprint {
                path: "schema.json".to_string(),
                algorithm: "sha256".to_string(),
                digest: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                    .to_string(),
            }
        );
    }

    #[test]
    fn endpoint_rejects_query_fragment_and_control_characters_in_paths() {
        for path in ["/users?active=true", "/users#section", "/users\n"] {
            let error = EndpointSelector::new("GET", path, 200)
                .validate()
                .unwrap_err()
                .to_string();
            assert!(error.contains("only the HTTP path"), "{error}");
        }
    }

    #[test]
    fn response_validation_rejects_bad_dialect_and_fingerprints() {
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);
        let mut response = ProviderResponse::new(
            ProviderIdentity::new("test", "1"),
            endpoint,
            json!({"$schema": JSON_SCHEMA_DRAFT_2020_12, "type": "object"}),
            vec![fingerprint_bytes("schema.json", b"schema")],
        );
        add_required_evidence(&mut response);
        assert!(validate_provider_response(&response).is_ok());

        response.schema["$schema"] = json!("http://json-schema.org/draft-07/schema#");
        assert!(
            validate_provider_response(&response)
                .unwrap_err()
                .to_string()
                .contains("$schema")
        );
        response.schema["$schema"] = json!(JSON_SCHEMA_DRAFT_2020_12);
        response.source_fingerprints[0].digest = "ABC".to_string();
        assert!(
            validate_provider_response(&response)
                .unwrap_err()
                .to_string()
                .contains("lowercase SHA-256")
        );
    }

    #[test]
    fn response_validation_does_not_treat_instance_data_as_subschemas() {
        let instance_data = json!({
            "$schema": "domain-value",
            "$ref": "https://example.test/domain-value",
            "nullable": true
        });
        let mut response = ProviderResponse::new(
            ProviderIdentity::new("test", "1"),
            EndpointSelector::new("GET", "/payload", 200),
            json!({
                "$schema": JSON_SCHEMA_DRAFT_2020_12,
                "type": "object",
                "examples": [instance_data.clone()],
                "properties": {
                    "payload": {"const": instance_data}
                }
            }),
            vec![fingerprint_bytes("schema.json", b"schema")],
        );
        add_required_evidence(&mut response);

        assert!(validate_provider_response(&response).is_ok());

        response.schema["properties"]["payload"]["$schema"] = json!("domain-value");
        let error = validate_provider_response(&response)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupported $schema"), "{error}");
        assert!(error.contains("#/properties/payload"), "{error}");
    }

    #[test]
    fn response_validation_rejects_ref_assertion_siblings_from_external_providers() {
        let mut response = ProviderResponse::new(
            ProviderIdentity::new("external.test", "1"),
            EndpointSelector::new("GET", "/identifier", 200),
            json!({
                "$schema": JSON_SCHEMA_DRAFT_2020_12,
                "$defs": {"identifier": {"type": "string"}},
                "$ref": "#/$defs/identifier",
                "type": "integer"
            }),
            vec![fingerprint_bytes("schema.json", b"schema")],
        );
        add_required_evidence(&mut response);

        let error = validate_provider_response(&response)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("assertion sibling keywords (type)"),
            "{error}"
        );
        assert!(error.contains("pre-dereference or flatten"), "{error}");
    }

    #[test]
    fn response_validation_requires_exact_typed_evidence_for_every_wire_claim() {
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);
        let mut response = ProviderResponse::new(
            ProviderIdentity::new("test", "1"),
            endpoint,
            json!({
                "$schema": JSON_SCHEMA_DRAFT_2020_12,
                "type": "object",
                "required": ["state"],
                "properties": {
                    "state": {"type": "string", "enum": ["ACTIVE"]},
                    "created_at": {"type": "string", "format": "date-time"},
                    "version": {"const": 1},
                    "nickname": {"type": ["string", "null"]},
                    "choice": {"anyOf": [{"type": "string"}, {"type": "null"}]},
                    "enum_null": {"enum": [null, "known"]},
                    "const_null": {"const": null}
                }
            }),
            vec![fingerprint_bytes("schema.json", b"schema")],
        );
        response.root_symbol = Some("UserEnvelope".to_string());
        add_required_evidence(&mut response);
        assert!(validate_provider_response(&response).is_ok());

        for (schema_path, kind) in [
            ("#", EVIDENCE_KIND_RESPONSE_WRAPPER),
            ("#", EVIDENCE_KIND_ROOT_SYMBOL),
            ("#/type", EVIDENCE_KIND_TYPE),
            ("#/required/0", EVIDENCE_KIND_REQUIREDNESS),
            ("#/properties/state", EVIDENCE_KIND_PROPERTY_NAME),
            ("#/properties/state/type", EVIDENCE_KIND_TYPE),
            ("#/properties/state/enum", EVIDENCE_KIND_ENUM),
            ("#/properties/created_at", EVIDENCE_KIND_REQUIREDNESS),
            ("#/properties/created_at/format", EVIDENCE_KIND_FORMAT),
            ("#/properties/version/const", EVIDENCE_KIND_CONST),
            ("#/properties/nickname/type/1", EVIDENCE_KIND_NULLABLE),
            ("#/properties/choice/anyOf/1/type", EVIDENCE_KIND_NULLABLE),
            ("#/properties/enum_null/enum/0", EVIDENCE_KIND_NULLABLE),
            ("#/properties/const_null/const", EVIDENCE_KIND_NULLABLE),
        ] {
            let mut invalid = response.clone();
            let claim = invalid
                .evidence
                .iter_mut()
                .find(|evidence| evidence.schema_path == schema_path && evidence.kind == kind)
                .unwrap_or_else(|| panic!("missing test evidence {kind} at {schema_path}"));
            claim.kind = "generic_schema_evidence".to_string();

            let error = validate_provider_response(&invalid)
                .unwrap_err()
                .to_string();
            assert!(error.contains(schema_path), "{error}");
            assert!(error.contains(kind), "{error}");
        }
    }

    #[test]
    fn response_validation_requires_exact_constraint_evidence() {
        let mut response = ProviderResponse::new(
            ProviderIdentity::new("test", "1"),
            EndpointSelector::new("GET", "/users", 200),
            json!({
                "$schema": JSON_SCHEMA_DRAFT_2020_12,
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": {"type": "string", "minLength": 1, "maxLength": 80},
                    "age": {"type": "integer", "minimum": 0},
                    "tags": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {"type": "string"}
                    }
                }
            }),
            vec![fingerprint_bytes("schema.json", b"schema")],
        );
        add_required_evidence(&mut response);
        assert!(validate_provider_response(&response).is_ok());

        for schema_path in [
            "#/additionalProperties",
            "#/properties/name/minLength",
            "#/properties/name/maxLength",
            "#/properties/age/minimum",
            "#/properties/tags/minItems",
            "#/properties/tags/uniqueItems",
        ] {
            let mut invalid = response.clone();
            let claim = invalid
                .evidence
                .iter_mut()
                .find(|evidence| {
                    evidence.schema_path == schema_path && evidence.kind == EVIDENCE_KIND_CONSTRAINT
                })
                .unwrap_or_else(|| panic!("missing constraint evidence at {schema_path}"));
            claim.kind = "generic_schema_evidence".to_string();

            let error = validate_provider_response(&invalid)
                .unwrap_err()
                .to_string();
            assert!(error.contains(schema_path), "{error}");
            assert!(error.contains(EVIDENCE_KIND_CONSTRAINT), "{error}");
        }
    }
}
