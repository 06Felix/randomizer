use std::collections::HashMap;

use crate::{
    generator::{FloatGenerator, Generator, IntGenerator, ObjectGenerator},
    schema::Schema,
};

pub fn compile_schema(schema: &Schema) -> Result<Generator, String> {
    match schema {
        Schema::Int { min, max } => {
            let min = min.unwrap_or(0);
            let max = max.unwrap_or(100);
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
            let mut fields = HashMap::new();
            for (key, value) in properties {
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

                fields.insert(key.clone(), generator);
            }
            Ok(Generator::Object(ObjectGenerator::new(fields)))
        }
    }
}
