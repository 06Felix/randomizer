use std::sync::Arc;

use tracing::debug;

use crate::{
    error::CompileError,
    generator::{
        BooleanGenerator, FloatGenerator, Generator, IntGenerator, ListGenerator, ObjectGenerator,
        PrimitiveEnumGenerator, StringGenerator, StringGeneratorMode, UUIDGenerator,
    },
    schema::{Schema, StringKind},
};

const ABSOLUTE_MAX_LENGTH: usize = 100;
pub const MAX_FLOAT_PRECISION: u8 = 9;
const ALPHABETIC_CHARSET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const NUMERIC_CHARSET: &str = "0123456789";
const ALPHANUMERIC_CHARSET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// Compiles a parsed schema into an executable generator tree.
///
/// Returns an error when a schema contains invalid bounds.
pub fn compile_schema(schema: &Schema) -> Result<Generator, CompileError> {
    debug!(schema = ?schema, "compiling schema");

    match schema {
        Schema::Int { min, max } => {
            let min = min.unwrap_or(i32::MIN);
            let max = max.unwrap_or(i32::MAX);
            if min > max {
                return Err(CompileError::InvalidRange {
                    min: min.to_string(),
                    max: max.to_string(),
                });
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
                return Err(CompileError::InvalidRange {
                    min: min.to_string(),
                    max: max.to_string(),
                });
            }
            if precision > MAX_FLOAT_PRECISION {
                return Err(CompileError::InvalidPrecision {
                    precision,
                    maximum: MAX_FLOAT_PRECISION,
                });
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
        } => compile_string_schema(
            *length,
            *min_length,
            *max_length,
            prefix,
            suffix,
            string_type,
            custom_charset,
        ),
        Schema::Enum { values } => compile_primitive_enum_schema(values),
        Schema::Object { properties } => {
            let mut entries: Vec<_> = properties.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));

            let mut fields = Vec::with_capacity(properties.len());
            for (key, value) in entries {
                let generator = compile_schema(value).map_err(|error| error.at_path(key))?;

                fields.push((Arc::from(key.as_str()), generator));
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
            let item_generator =
                Box::new(compile_schema(items).map_err(|error| error.at_path("items"))?);

            Ok(Generator::List(ListGenerator {
                min_length,
                max_length,
                item_generator,
            }))
        }
        Schema::Boolean { true_probability } => {
            let true_probability = u8::try_from(*true_probability)
                .ok()
                .filter(|probability| *probability <= 100)
                .ok_or(CompileError::InvalidProbability(*true_probability))?;
            Ok(Generator::Boolean(BooleanGenerator { true_probability }))
        }
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
) -> Result<Generator, CompileError> {
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
                return Err(CompileError::MissingCustomCharset);
            };
            if custom_charset.is_empty() {
                return Err(CompileError::EmptyCustomCharset);
            }

            StringGeneratorMode::Charset {
                min_length,
                max_length,
                charset: custom_charset.chars().collect(),
            }
        }
    };

    Ok(Generator::String(StringGenerator {
        prefix,
        suffix,
        mode,
    }))
}

fn compile_primitive_enum_schema(values: &[serde_json::Value]) -> Result<Generator, CompileError> {
    if values.is_empty() {
        return Err(CompileError::EmptyEnum);
    }

    for value in values {
        if !matches!(
            value,
            serde_json::Value::String(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::Bool(_)
        ) {
            return Err(CompileError::InvalidEnumValue);
        }
    }

    Ok(Generator::Enum(PrimitiveEnumGenerator {
        values: values.to_vec(),
    }))
}

fn resolve_length_range(
    length: Option<usize>,
    min_length: Option<usize>,
    max_length: Option<usize>,
    subject: &'static str,
) -> Result<(usize, usize), CompileError> {
    if let Some(length) = length {
        validate_length(length, subject)?;
        return Ok((length, length));
    }

    match (min_length, max_length) {
        (Some(min_length), Some(max_length)) => {
            validate_length(min_length, subject)?;
            validate_length(max_length, subject)?;

            if min_length > max_length {
                return Err(CompileError::InvalidLengthRange { subject });
            }

            Ok((min_length, max_length))
        }
        _ => Err(CompileError::MissingLength { subject }),
    }
}

fn validate_length(length: usize, subject: &'static str) -> Result<(), CompileError> {
    if length > ABSOLUTE_MAX_LENGTH {
        return Err(CompileError::LengthTooLarge {
            subject,
            maximum: ABSOLUTE_MAX_LENGTH,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;

    #[test]
    fn rejects_invalid_numeric_constraints() {
        assert!(matches!(
            compile_schema(&Schema::Int {
                min: Some(2),
                max: Some(1),
            }),
            Err(CompileError::InvalidRange { .. })
        ));
        assert!(matches!(
            compile_schema(&Schema::Float {
                min: None,
                max: None,
                precision: Some(MAX_FLOAT_PRECISION + 1),
            }),
            Err(CompileError::InvalidPrecision { .. })
        ));
    }

    #[test]
    fn rejects_probability_outside_percentage_range() {
        for probability in [-1, 101] {
            assert_eq!(
                compile_schema(&Schema::Boolean {
                    true_probability: probability,
                })
                .unwrap_err(),
                CompileError::InvalidProbability(probability)
            );
        }
    }

    #[test]
    fn rejects_invalid_string_enum_and_list_constraints() {
        assert_eq!(
            compile_schema(&Schema::String {
                length: Some(1),
                min_length: None,
                max_length: None,
                prefix: None,
                suffix: None,
                string_type: StringKind::Custom,
                custom_charset: None,
            })
            .unwrap_err(),
            CompileError::MissingCustomCharset
        );
        assert_eq!(
            compile_schema(&Schema::Enum { values: vec![] }).unwrap_err(),
            CompileError::EmptyEnum
        );
        assert_eq!(
            compile_schema(&Schema::Enum {
                values: vec![json!(null)],
            })
            .unwrap_err(),
            CompileError::InvalidEnumValue
        );
        assert!(matches!(
            compile_schema(&Schema::List {
                length: None,
                min_length: Some(3),
                max_length: Some(2),
                items: Box::new(Schema::Boolean {
                    true_probability: 50,
                }),
            }),
            Err(CompileError::InvalidLengthRange { subject: "list" })
        ));
    }

    #[test]
    fn adds_nested_property_path_to_errors() {
        let schema = Schema::Object {
            properties: HashMap::from([(
                "score".to_string(),
                Schema::Float {
                    min: None,
                    max: None,
                    precision: Some(10),
                },
            )]),
        };
        assert_eq!(
            compile_schema(&schema).unwrap_err().to_string(),
            "score: precision 10 exceeds the maximum supported precision 9"
        );
    }
}
