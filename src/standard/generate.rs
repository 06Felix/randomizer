use std::collections::BTreeSet;

use rand::RngExt;
use rand_regex::Regex;
use serde_json::{Map, Number, Value, json};

use crate::{
    error::StandardContractError,
    generation::{GenerationMode, StableRng, ViolatedRule},
    standard::ImportedContract,
};

const DEFAULT_STRING_LENGTH: usize = 12;
const DEFAULT_ARRAY_LENGTH: usize = 2;
const MAX_GENERATED_LENGTH: usize = 100;

pub(super) fn generate(
    schema: &Value,
    root: &Value,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
    references: &mut Vec<String>,
) -> Result<Value, StandardContractError> {
    if let Some(value) = schema.get("const") {
        return Ok(value.clone());
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if values.is_empty() {
            return unsupported(path, "enum must contain at least one value");
        }
        let index = select_index(values.len(), mode, rng);
        return Ok(values[index].clone());
    }
    if mode == GenerationMode::Example
        && let Some(example) = first_example(schema)
    {
        return Ok(example.clone());
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return generate_reference(reference, root, mode, rng, path, references);
    }
    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        return generate_branch(branches, root, mode, rng, path, references, "oneOf");
    }
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        return generate_branch(branches, root, mode, rng, path, references, "anyOf");
    }

    let types = schema_types(schema);
    if types.contains(&"null") && (types.len() == 1 || mode == GenerationMode::Minimum) {
        return Ok(Value::Null);
    }
    let selected_type = types
        .iter()
        .find(|value| **value != "null")
        .copied()
        .or_else(|| infer_type(schema));

    match selected_type {
        Some("object") => generate_object(schema, root, mode, rng, path, references),
        Some("array") => generate_array(schema, root, mode, rng, path, references),
        Some("string") => generate_string(schema, mode, rng, path),
        Some("integer") => generate_number(schema, mode, rng, path, true),
        Some("number") => generate_number(schema, mode, rng, path, false),
        Some("boolean") => Ok(Value::Bool(match mode {
            GenerationMode::Minimum => false,
            GenerationMode::Maximum => true,
            _ => rng.below(2) == 1,
        })),
        Some("null") | None => Ok(Value::Null),
        Some(other) => unsupported(path, format!("unsupported type {other:?}")),
    }
}

fn generate_reference(
    reference: &str,
    root: &Value,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
    references: &mut Vec<String>,
) -> Result<Value, StandardContractError> {
    if references.iter().any(|seen| seen == reference) {
        return Err(StandardContractError::CyclicReference {
            reference: reference.to_string(),
        });
    }
    let pointer =
        reference
            .strip_prefix('#')
            .ok_or_else(|| StandardContractError::ExternalReference {
                reference: reference.to_string(),
            })?;
    let resolved =
        root.pointer(pointer)
            .ok_or_else(|| StandardContractError::UnresolvedReference {
                reference: reference.to_string(),
            })?;
    references.push(reference.to_string());
    let result = generate(resolved, root, mode, rng, path, references);
    references.pop();
    result
}

fn generate_branch(
    branches: &[Value],
    root: &Value,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
    references: &mut Vec<String>,
    keyword: &str,
) -> Result<Value, StandardContractError> {
    if branches.is_empty() {
        return unsupported(path, format!("{keyword} must contain at least one schema"));
    }
    let index = select_index(branches.len(), mode, rng);
    generate(
        &branches[index],
        root,
        mode,
        rng,
        &format!("{path}/{keyword}/{index}"),
        references,
    )
}

fn generate_object(
    schema: &Value,
    root: &Value,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
    references: &mut Vec<String>,
) -> Result<Value, StandardContractError> {
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required: BTreeSet<_> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut keys: Vec<_> = properties.keys().cloned().collect();
    keys.sort();
    let mut object = Map::new();

    for key in keys {
        let is_required = required.contains(key.as_str());
        let include_optional = match mode {
            GenerationMode::Minimum => false,
            GenerationMode::Maximum | GenerationMode::Example => true,
            GenerationMode::Boundary | GenerationMode::Valid => rng.below(2) == 1,
            GenerationMode::Invalid => unreachable!("invalid generation starts from valid mode"),
        };
        if is_required || include_optional {
            let property_schema = &properties[&key];
            let value = generate(
                property_schema,
                root,
                mode,
                rng,
                &format!("{path}/properties/{}", escape_pointer(&key)),
                references,
            )?;
            object.insert(key, value);
        }
    }

    if let Some(missing) = required.iter().find(|key| !properties.contains_key(**key)) {
        return unsupported(
            path,
            format!("required property {missing:?} has no generation schema"),
        );
    }
    Ok(Value::Object(object))
}

fn generate_array(
    schema: &Value,
    root: &Value,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
    references: &mut Vec<String>,
) -> Result<Value, StandardContractError> {
    let min = usize_keyword(schema, "minItems").unwrap_or(0);
    let max = usize_keyword(schema, "maxItems")
        .unwrap_or_else(|| min.max(DEFAULT_ARRAY_LENGTH))
        .min(MAX_GENERATED_LENGTH);
    if min > max {
        return unsupported(
            path,
            "minItems is greater than maxItems or generation limit",
        );
    }
    let length = select_range(min, max, mode, rng);
    let item_schema = schema.get("items").unwrap_or(&Value::Bool(true));
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        values.push(generate(
            item_schema,
            root,
            mode,
            rng,
            &format!("{path}/items/{index}"),
            references,
        )?);
    }
    Ok(Value::Array(values))
}

fn generate_string(
    schema: &Value,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
) -> Result<Value, StandardContractError> {
    if mode == GenerationMode::Example
        && let Some(example) = first_example(schema).and_then(Value::as_str)
    {
        return Ok(Value::String(example.to_string()));
    }
    let min = usize_keyword(schema, "minLength").unwrap_or(0);
    let configured_max = usize_keyword(schema, "maxLength");
    let max = configured_max
        .unwrap_or(MAX_GENERATED_LENGTH)
        .min(MAX_GENERATED_LENGTH);
    if min > max {
        return unsupported(
            path,
            "minLength is greater than maxLength or generation limit",
        );
    }
    if let Some(format) = schema.get("format").and_then(Value::as_str) {
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(schema)
            .map_err(|error| StandardContractError::UnsupportedGeneration {
                schema_path: path.to_string(),
                reason: error.to_string(),
            })?;
        for _ in 0..32 {
            let candidate = Value::String(generate_format(format, mode, rng, path)?);
            if validator.is_valid(&candidate) {
                return Ok(candidate);
            }
        }
        return unsupported(
            path,
            format!("format {format:?} could not satisfy its other string constraints"),
        );
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        let repeat_limit = max.max(1) as u32;
        let generation_pattern = pattern
            .strip_prefix('^')
            .unwrap_or(pattern)
            .strip_suffix('$')
            .unwrap_or_else(|| pattern.strip_prefix('^').unwrap_or(pattern));
        let regex = Regex::compile(generation_pattern, repeat_limit).map_err(|error| {
            StandardContractError::UnsupportedGeneration {
                schema_path: path.to_string(),
                reason: format!("pattern {pattern:?} cannot be generated: {error}"),
            }
        })?;
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(schema)
            .map_err(|error| StandardContractError::UnsupportedGeneration {
                schema_path: path.to_string(),
                reason: error.to_string(),
            })?;
        for _ in 0..32 {
            let candidate = rng.sample::<String, _>(&regex);
            let candidate = Value::String(candidate);
            if validator.is_valid(&candidate) {
                return Ok(candidate);
            }
        }
        return unsupported(path, "pattern could not satisfy length constraints");
    }

    let selection_max = configured_max
        .unwrap_or_else(|| min.max(DEFAULT_STRING_LENGTH))
        .min(max);
    let length = select_range(min, selection_max, mode, rng);
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let value: String = (0..length)
        .map(|_| CHARSET[rng.usize_inclusive(0, CHARSET.len() - 1)] as char)
        .collect();
    Ok(Value::String(value))
}

fn generate_format(
    format: &str,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
) -> Result<String, StandardContractError> {
    let value = match format {
        "date" => match mode {
            GenerationMode::Minimum => "1970-01-01".to_string(),
            GenerationMode::Maximum => "9999-12-31".to_string(),
            _ => format!("2024-01-{:02}", rng.usize_inclusive(1, 28)),
        },
        "date-time" => match mode {
            GenerationMode::Minimum => "1970-01-01T00:00:00Z".to_string(),
            GenerationMode::Maximum => "9999-12-31T23:59:59Z".to_string(),
            _ => format!(
                "2024-01-{:02}T{:02}:{:02}:{:02}Z",
                rng.usize_inclusive(1, 28),
                rng.usize_inclusive(0, 23),
                rng.usize_inclusive(0, 59),
                rng.usize_inclusive(0, 59)
            ),
        },
        "email" => format!("user{}@example.com", rng.below(1_000_000)),
        "uri" | "uri-reference" => {
            format!("https://example.com/resources/{}", rng.below(1_000_000))
        }
        "uuid" => deterministic_uuid(rng).to_string(),
        other => return unsupported(path, format!("unsupported format {other:?}")),
    };
    Ok(value)
}

fn generate_number(
    schema: &Value,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
    integer: bool,
) -> Result<Value, StandardContractError> {
    let mut min = schema
        .get("minimum")
        .and_then(Value::as_f64)
        .unwrap_or(-100.0);
    let mut max = schema
        .get("maximum")
        .and_then(Value::as_f64)
        .unwrap_or(100.0);
    if let Some(exclusive) = schema.get("exclusiveMinimum").and_then(Value::as_f64) {
        min = if integer {
            exclusive.floor() + 1.0
        } else {
            next_up(exclusive)
        };
    }
    if let Some(exclusive) = schema.get("exclusiveMaximum").and_then(Value::as_f64) {
        max = if integer {
            exclusive.ceil() - 1.0
        } else {
            next_down(exclusive)
        };
    }
    if min > max || !min.is_finite() || !max.is_finite() {
        return unsupported(path, "numeric range is empty or non-finite");
    }

    let multiple = schema.get("multipleOf").and_then(Value::as_f64);
    if let Some(multiple) = multiple
        && (multiple <= 0.0 || !multiple.is_finite())
    {
        return unsupported(path, "multipleOf must be a positive finite number");
    }
    if integer {
        return generate_integer_number(min, max, multiple, mode, rng, path);
    }
    if let Some(multiple) = multiple {
        min = (min / multiple).ceil() * multiple;
        max = (max / multiple).floor() * multiple;
        if min > max {
            return unsupported(path, "numeric range contains no multipleOf value");
        }
    }

    let mut value = match mode {
        GenerationMode::Minimum => min,
        GenerationMode::Maximum => max,
        GenerationMode::Boundary => {
            if rng.below(2) == 0 {
                min
            } else {
                max
            }
        }
        _ => min + ((rng.next_u64() >> 11) as f64 / ((1_u64 << 53) - 1) as f64) * (max - min),
    };
    if let Some(multiple) = multiple {
        value = (value / multiple).round() * multiple;
        value = value.clamp(min, max);
    }
    Number::from_f64(value).map(Value::Number).ok_or_else(|| {
        StandardContractError::UnsupportedGeneration {
            schema_path: path.to_string(),
            reason: "generated number is not representable in JSON".to_string(),
        }
    })
}

fn generate_integer_number(
    min: f64,
    max: f64,
    multiple: Option<f64>,
    mode: GenerationMode,
    rng: &mut StableRng,
    path: &str,
) -> Result<Value, StandardContractError> {
    let low = min.ceil();
    let high = max.floor();
    if low > high || low < i64::MIN as f64 || high > i64::MAX as f64 {
        return unsupported(
            path,
            "integer range is empty or outside signed 64-bit support",
        );
    }
    let low = low as i64;
    let high = high as i64;
    let width = (i128::from(high) - i128::from(low) + 1) as u128;
    let random_offset = if width > u128::from(u64::MAX) {
        u128::from(rng.next_u64())
    } else {
        u128::from(rng.below(width as u64))
    };
    let (start, direction) = match mode {
        GenerationMode::Minimum => (low, 1_i8),
        GenerationMode::Maximum => (high, -1),
        GenerationMode::Boundary if rng.below(2) == 0 => (low, 1),
        GenerationMode::Boundary => (high, -1),
        _ => ((i128::from(low) + random_offset as i128) as i64, 1),
    };

    let attempts = width.min(1_000_000) as usize;
    for offset in 0..attempts {
        let delta = offset as i128 * i128::from(direction);
        let mut candidate = i128::from(start) + delta;
        if candidate > i128::from(high) {
            candidate = i128::from(low) + (candidate - i128::from(high) - 1);
        } else if candidate < i128::from(low) {
            candidate = i128::from(high) - (i128::from(low) - candidate - 1);
        }
        let candidate = candidate as i64;
        if multiple.is_none_or(|multiple| is_multiple(candidate as f64, multiple)) {
            return Ok(Value::Number(Number::from(candidate)));
        }
    }
    unsupported(path, "integer range contains no supported multipleOf value")
}

fn is_multiple(value: f64, multiple: f64) -> bool {
    let quotient = value / multiple;
    (quotient - quotient.round()).abs() <= f64::EPSILON * quotient.abs().max(1.0) * 4.0
}

pub(super) fn make_invalid(
    schema: &Value,
    valid: &Value,
    contract: &ImportedContract,
) -> Result<(Value, ViolatedRule), StandardContractError> {
    let mut candidates = Vec::new();
    collect_invalid_candidates(schema, schema, valid, "", &mut candidates, &mut Vec::new())?;
    for candidate in candidates {
        let report = contract.validate(&candidate);
        if let Some(violation) = report.violations.into_iter().next() {
            return Ok((candidate, violation));
        }
    }
    Err(StandardContractError::UnableToProduceInvalidValue)
}

fn collect_invalid_candidates(
    schema: &Value,
    root: &Value,
    valid: &Value,
    instance_path: &str,
    candidates: &mut Vec<Value>,
    references: &mut Vec<String>,
) -> Result<(), StandardContractError> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        if references.iter().any(|seen| seen == reference) {
            return Ok(());
        }
        let resolved = root.pointer(reference.strip_prefix('#').unwrap_or(""));
        if let Some(resolved) = resolved {
            references.push(reference.to_string());
            collect_invalid_candidates(
                resolved,
                root,
                valid,
                instance_path,
                candidates,
                references,
            )?;
            references.pop();
        }
        return Ok(());
    }

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for property in required.iter().filter_map(Value::as_str) {
            if let Some(Value::Object(object)) = value_at(valid, instance_path)
                && object.contains_key(property)
            {
                let mut candidate = valid.clone();
                if let Some(Value::Object(target)) = value_at_mut(&mut candidate, instance_path) {
                    target.remove(property);
                    candidates.push(candidate);
                }
            }
        }
    }

    if value_at(valid, instance_path).is_some() {
        for replacement in invalid_replacements(schema) {
            let mut candidate = valid.clone();
            if instance_path.is_empty() {
                candidate = replacement;
            } else if let Some(target) = value_at_mut(&mut candidate, instance_path) {
                *target = replacement;
            }
            candidates.push(candidate);
        }
    }

    for keyword in ["oneOf", "anyOf"] {
        if let Some(branches) = schema.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                collect_invalid_candidates(
                    branch,
                    root,
                    valid,
                    instance_path,
                    candidates,
                    references,
                )?;
            }
        }
    }

    if let (Some(properties), Some(Value::Object(object))) = (
        schema.get("properties").and_then(Value::as_object),
        value_at(valid, instance_path),
    ) {
        let mut keys: Vec<_> = properties.keys().collect();
        keys.sort();
        for key in keys {
            if object.contains_key(key) {
                collect_invalid_candidates(
                    &properties[key],
                    root,
                    valid,
                    &format!("{instance_path}/{}", escape_pointer(key)),
                    candidates,
                    references,
                )?;
            }
        }
    }
    if let (Some(items), Some(Value::Array(values))) =
        (schema.get("items"), value_at(valid, instance_path))
    {
        for index in 0..values.len() {
            collect_invalid_candidates(
                items,
                root,
                valid,
                &format!("{instance_path}/{index}"),
                candidates,
                references,
            )?;
        }
    }
    Ok(())
}

fn invalid_replacements(schema: &Value) -> Vec<Value> {
    let mut values = Vec::new();
    if schema.get("const").is_some() || schema.get("enum").is_some() {
        values.push(json!({"__randomizer_invalid": true}));
    }
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        values.push(json!(minimum - 1.0));
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64) {
        values.push(json!(maximum + 1.0));
    }
    if schema.get("pattern").is_some() || schema.get("format").is_some() {
        values.push(Value::String("__not_valid__".to_string()));
    }
    if schema.get("minLength").and_then(Value::as_u64).unwrap_or(0) > 0 {
        values.push(Value::String(String::new()));
    }
    if let Some(max) = schema.get("maxLength").and_then(Value::as_u64)
        && max < MAX_GENERATED_LENGTH as u64
    {
        values.push(Value::String("x".repeat(max as usize + 1)));
    }
    let types = schema_types(schema);
    if !types.is_empty() {
        let replacement = if types.contains(&"object") {
            Value::String("not-an-object".to_string())
        } else if types.contains(&"string") || types.contains(&"array") {
            json!({})
        } else if types.contains(&"boolean") {
            Value::String("not-a-boolean".to_string())
        } else {
            Value::String("not-a-number".to_string())
        };
        values.push(replacement);
    }
    values
}

fn schema_types(schema: &Value) -> Vec<&str> {
    match schema.get("type") {
        Some(Value::String(value)) => vec![value.as_str()],
        Some(Value::Array(values)) => values.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn infer_type(schema: &Value) -> Option<&'static str> {
    if schema.get("properties").is_some() || schema.get("required").is_some() {
        Some("object")
    } else if schema.get("items").is_some() {
        Some("array")
    } else if schema.get("format").is_some()
        || schema.get("pattern").is_some()
        || schema.get("minLength").is_some()
    {
        Some("string")
    } else if schema.get("minimum").is_some() || schema.get("maximum").is_some() {
        Some("number")
    } else {
        None
    }
}

fn first_example(schema: &Value) -> Option<&Value> {
    schema
        .get("examples")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .or_else(|| schema.get("example"))
}

fn select_index(length: usize, mode: GenerationMode, rng: &mut StableRng) -> usize {
    match mode {
        GenerationMode::Minimum => 0,
        GenerationMode::Maximum => length - 1,
        _ => rng.usize_inclusive(0, length - 1),
    }
}

fn select_range(min: usize, max: usize, mode: GenerationMode, rng: &mut StableRng) -> usize {
    match mode {
        GenerationMode::Minimum => min,
        GenerationMode::Maximum => max,
        GenerationMode::Boundary => {
            if rng.below(2) == 0 {
                min
            } else {
                max
            }
        }
        _ => rng.usize_inclusive(min, max),
    }
}

fn usize_keyword(schema: &Value, keyword: &str) -> Option<usize> {
    schema
        .get(keyword)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn deterministic_uuid(rng: &mut StableRng) -> uuid::Uuid {
    let mut bytes = [0_u8; 16];
    rng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

fn next_up(value: f64) -> f64 {
    if value.is_nan() || value == f64::INFINITY {
        return value;
    }
    if value == -0.0 {
        return f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value >= 0.0 { bits + 1 } else { bits - 1 })
}

fn next_down(value: f64) -> f64 {
    if value.is_nan() || value == f64::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value > 0.0 { bits - 1 } else { bits + 1 })
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn value_at<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    if pointer.is_empty() {
        Some(value)
    } else {
        value.pointer(pointer)
    }
}

fn value_at_mut<'a>(value: &'a mut Value, pointer: &str) -> Option<&'a mut Value> {
    if pointer.is_empty() {
        Some(value)
    } else {
        value.pointer_mut(pointer)
    }
}

fn unsupported<T>(path: &str, reason: impl Into<String>) -> Result<T, StandardContractError> {
    Err(StandardContractError::UnsupportedGeneration {
        schema_path: path.to_string(),
        reason: reason.into(),
    })
}
