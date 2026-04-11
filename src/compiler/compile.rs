use std::sync::Arc;

use tracing::debug;

use crate::{
    generator::{
        BooleanGenerator, FloatGenerator, Generator, IntGenerator, ListGenerator, ObjectGenerator,
        StringGenerator, StringGeneratorMode, UUIDGenerator,
    },
    schema::{Schema, StringKind},
};

const ABSOLUTE_MAX_LENGTH: usize = 100;
const ALPHABETIC_CHARSET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const NUMERIC_CHARSET: &str = "0123456789";
const ALPHANUMERIC_CHARSET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

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
        Schema::String {
            length,
            min_length,
            max_length,
            prefix,
            suffix,
            string_type,
            custom_charset,
            enum_values,
        } => compile_string_schema(
            *length,
            *min_length,
            *max_length,
            prefix,
            suffix,
            string_type,
            custom_charset,
            enum_values,
        ),
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
            let (min_length, max_length) =
                resolve_length_range(*length, *min_length, *max_length, "list")?;
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

fn compile_string_schema(
    length: Option<usize>,
    min_length: Option<usize>,
    max_length: Option<usize>,
    prefix: &Option<String>,
    suffix: &Option<String>,
    string_type: &StringKind,
    custom_charset: &Option<String>,
    enum_values: &Option<Vec<String>>,
) -> Result<Generator, String> {
    let prefix = prefix.clone().unwrap_or_default();
    let suffix = suffix.clone().unwrap_or_default();

    let mode = match string_type {
        StringKind::Alphabetic => {
            let (min_length, max_length) =
                resolve_length_range(length, min_length, max_length, "string")?;
            StringGeneratorMode::Charset {
                min_length,
                max_length,
                charset: ALPHABETIC_CHARSET.chars().collect(),
            }
        }
        StringKind::Numeric => {
            let (min_length, max_length) =
                resolve_length_range(length, min_length, max_length, "string")?;
            StringGeneratorMode::Charset {
                min_length,
                max_length,
                charset: NUMERIC_CHARSET.chars().collect(),
            }
        }
        StringKind::Alphanumeric => {
            let (min_length, max_length) =
                resolve_length_range(length, min_length, max_length, "string")?;
            StringGeneratorMode::Charset {
                min_length,
                max_length,
                charset: ALPHANUMERIC_CHARSET.chars().collect(),
            }
        }
        StringKind::Custom => {
            let (min_length, max_length) =
                resolve_length_range(length, min_length, max_length, "string")?;
            let Some(custom_charset) = custom_charset else {
                return Err("Error: custom strings require custom_charset".to_string());
            };
            if custom_charset.is_empty() {
                return Err("Error: custom_charset cannot be empty".to_string());
            }

            StringGeneratorMode::Charset {
                min_length,
                max_length,
                charset: custom_charset.chars().collect(),
            }
        }
        StringKind::Enum => {
            let Some(enum_values) = enum_values else {
                return Err("Error: enum strings require enum_values".to_string());
            };
            if enum_values.is_empty() {
                return Err("Error: enum_values cannot be empty".to_string());
            }

            StringGeneratorMode::Enum {
                values: enum_values.clone(),
            }
        }
    };

    Ok(Generator::String(StringGenerator {
        prefix,
        suffix,
        mode,
    }))
}

fn resolve_length_range(
    length: Option<usize>,
    min_length: Option<usize>,
    max_length: Option<usize>,
    subject: &str,
) -> Result<(usize, usize), String> {
    if let Some(length) = length {
        validate_length(length, subject)?;
        return Ok((length, length));
    }

    match (min_length, max_length) {
        (Some(min_length), Some(max_length)) => {
            validate_length(min_length, subject)?;
            validate_length(max_length, subject)?;

            if min_length > max_length {
                return Err("Error: min_length is greater than max_length".to_string());
            }

            Ok((min_length, max_length))
        }
        _ => Err("Error: provide either length or both min_length and max_length".to_string()),
    }
}

fn validate_length(length: usize, subject: &str) -> Result<(), String> {
    if length > ABSOLUTE_MAX_LENGTH {
        return Err(format!(
            "Error: {} length cannot exceed {}",
            subject, ABSOLUTE_MAX_LENGTH
        ));
    }

    Ok(())
}
