use std::sync::Arc;

use tracing::debug;

use crate::{
    generator::{BooleanGenerator, FloatGenerator, Generator, IntGenerator, ObjectGenerator},
    schema::Schema,
};

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
        Schema::Boolean { true_probability } => Ok(Generator::Boolean(BooleanGenerator {
            true_probability: *true_probability,
        })),
    }
}
