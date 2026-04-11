use std::sync::Arc;

use tracing::debug;

use crate::{
    generator::{
        BooleanGenerator, FloatGenerator, Generator, IntGenerator, ListGenerator, ObjectGenerator,
        UUIDGenerator,
    },
    schema::Schema,
};

const ABSOLUTE_MAX_LENGTH: usize = 100;

/// Compiles a parsed schema into an executable generator tree.
///
/// Returns an error when a schema contains invalid bounds.
pub fn compile_schema(schema: &Schema) -> Result<Generator, String> {
    debug!(schema = ?schema, "compiling schema");

    match schema {
        Schema::Int { min, max } => {
            let min = min.unwrap_or(i32::MIN);
            let max = max.unwrap_or(i32::MAX);
            if min > max {
                return Err("Error: min is greater than max".to_string());
            }
            Ok(Generator::Int(IntGenerator { min, max }))
        }
        Schema::Float {
            min,
            max,
            precision,
        } => {
            let min = min.unwrap_or(0.0);
            let max = max.unwrap_or(1.0);
            let precision = precision.unwrap_or(2);
            if min > max {
                return Err("Error: min is greater than max".to_string());
            }
            Ok(Generator::Float(FloatGenerator {
                min,
                max,
                precision,
            }))
        }
        Schema::Object { properties } => {
            let mut keys: Vec<String> = properties.keys().cloned().collect();
            keys.sort();

            let mut fields = Vec::with_capacity(properties.len());
            for key in keys {
                let Some(value) = properties.get(&key) else {
                    return Err(format!(
                        "internal compile error: property {key:?} missing from schema map"
                    ));
                };
                let generator = compile_schema(value).map_err(|e| {
                    let e_str = e.to_string();

                    if e_str.starts_with("Error: ") {
                        format!(
                            "{}: {}",
                            key,
                            e_str.strip_prefix("Error: ").unwrap_or(&e_str)
                        )
                    } else {
                        format!("Error in {}.{}", key, e_str)
                    }
                })?;

                fields.push((Arc::from(key.into_boxed_str()), generator));
            }
            Ok(Generator::Object(ObjectGenerator { fields }))
        }
        Schema::List {
            length,
            min_length,
            max_length,
            items,
        } => {
            let (min_length, max_length) = resolve_list_length(*length, *min_length, *max_length)?;
            let item_generator = Box::new(compile_schema(items).map_err(|e| {
                let e_str = e.to_string();

                if e_str.starts_with("Error: ") {
                    format!("items: {}", e_str.strip_prefix("Error: ").unwrap_or(&e_str))
                } else {
                    format!("Error in items.{}", e_str)
                }
            })?);

            Ok(Generator::List(ListGenerator {
                min_length,
                max_length,
                item_generator,
            }))
        }
        Schema::Boolean { true_probability } => Ok(Generator::Boolean(BooleanGenerator {
            true_probability: *true_probability,
        })),
        Schema::Uuid { prefix, suffix } => Ok(Generator::Uuid(UUIDGenerator {
            prefix: prefix.clone().unwrap_or_default(),
            suffix: suffix.clone().unwrap_or_default(),
        })),
    }
}

fn resolve_list_length(
    length: Option<usize>,
    min_length: Option<usize>,
    max_length: Option<usize>,
) -> Result<(usize, usize), String> {
    if let Some(length) = length {
        validate_list_length(length)?;
        return Ok((length, length));
    }

    match (min_length, max_length) {
        (Some(min_length), Some(max_length)) => {
            validate_list_length(min_length)?;
            validate_list_length(max_length)?;

            if min_length > max_length {
                return Err("Error: minLength is greater than maxLength".to_string());
            }

            Ok((min_length, max_length))
        }
        _ => Err("Error: provide either length or both minLength and maxLength".to_string()),
    }
}

fn validate_list_length(length: usize) -> Result<(), String> {
    if length > ABSOLUTE_MAX_LENGTH {
        return Err(format!(
            "Error: list length cannot exceed {}",
            ABSOLUTE_MAX_LENGTH
        ));
    }

    Ok(())
}
