use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SubschemaKind {
    Direct,
    Array,
    Map,
}

/// Return how a Draft 2020-12 keyword contains subschemas.
///
/// Values under annotations such as `examples`, `default`, `const`, and `enum`
/// are instance data, not schemas. Keeping this list explicit prevents payload
/// fields named `$schema`, `$ref`, or `nullable` from being interpreted as schema
/// control keywords.
pub(super) fn subschema_kind(keyword: &str) -> Option<SubschemaKind> {
    match keyword {
        "additionalProperties"
        | "contains"
        | "contentSchema"
        | "else"
        | "if"
        | "items"
        | "not"
        | "propertyNames"
        | "then"
        | "unevaluatedItems"
        | "unevaluatedProperties" => Some(SubschemaKind::Direct),
        "allOf" | "anyOf" | "oneOf" | "prefixItems" => Some(SubschemaKind::Array),
        "$defs" | "dependentSchemas" | "patternProperties" | "properties" => {
            Some(SubschemaKind::Map)
        }
        _ => None,
    }
}

pub(super) fn is_non_assertion_ref_sibling(keyword: &str) -> bool {
    matches!(
        keyword,
        "$schema"
            | "$id"
            | "$anchor"
            | "$dynamicAnchor"
            | "$vocabulary"
            | "$comment"
            | "$defs"
            | "title"
            | "description"
            | "default"
            | "deprecated"
            | "readOnly"
            | "writeOnly"
            | "examples"
    )
}

pub(super) fn walk_schema<E>(
    value: &Value,
    pointer: &str,
    visitor: &mut impl FnMut(&Value, &str) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    visitor(value, pointer)?;
    let Some(object) = value.as_object() else {
        return Ok(());
    };

    for (keyword, nested) in object {
        let Some(kind) = subschema_kind(keyword) else {
            continue;
        };
        let keyword_pointer = join_pointer(pointer, keyword);
        match kind {
            SubschemaKind::Direct => walk_schema(nested, &keyword_pointer, visitor)?,
            SubschemaKind::Array => {
                if let Some(values) = nested.as_array() {
                    for (index, subschema) in values.iter().enumerate() {
                        walk_schema(
                            subschema,
                            &join_pointer(&keyword_pointer, &index.to_string()),
                            visitor,
                        )?;
                    }
                }
            }
            SubschemaKind::Map => {
                if let Some(values) = nested.as_object() {
                    for (name, subschema) in values {
                        walk_schema(subschema, &join_pointer(&keyword_pointer, name), visitor)?;
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn walk_schema_mut<E>(
    value: &mut Value,
    pointer: &str,
    visitor: &mut impl FnMut(&mut Value, &str) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    visitor(value, pointer)?;
    let Some(object) = value.as_object_mut() else {
        return Ok(());
    };

    for keyword in [
        "additionalProperties",
        "contains",
        "contentSchema",
        "else",
        "if",
        "items",
        "not",
        "propertyNames",
        "then",
        "unevaluatedItems",
        "unevaluatedProperties",
    ] {
        if let Some(nested) = object.get_mut(keyword) {
            walk_schema_mut(nested, &join_pointer(pointer, keyword), visitor)?;
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(values) = object.get_mut(keyword).and_then(Value::as_array_mut) {
            for (index, subschema) in values.iter_mut().enumerate() {
                walk_schema_mut(
                    subschema,
                    &join_pointer(&join_pointer(pointer, keyword), &index.to_string()),
                    visitor,
                )?;
            }
        }
    }
    for keyword in [
        "$defs",
        "dependentSchemas",
        "patternProperties",
        "properties",
    ] {
        if let Some(values) = object.get_mut(keyword).and_then(Value::as_object_mut) {
            for (name, subschema) in values {
                walk_schema_mut(
                    subschema,
                    &join_pointer(&join_pointer(pointer, keyword), name),
                    visitor,
                )?;
            }
        }
    }
    Ok(())
}

fn join_pointer(base: &str, token: &str) -> String {
    format!("{base}/{}", token.replace('~', "~0").replace('/', "~1"))
}
