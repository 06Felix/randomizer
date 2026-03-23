use std::collections::HashMap;

use crate::{
    generator::{FloatGenerator, Generator, IntGenerator, ObjectGenerator},
    schema::Schema,
};

pub fn compile_schema(schema: &Schema) -> Generator {
    match schema {
        Schema::Int { min, max } => Generator::Int(IntGenerator::new(*min, *max)),
        Schema::Float {
            min,
            max,
            precision,
        } => Generator::Float(FloatGenerator::new(*min, *max, *precision)),
        Schema::Object { properties } => {
            let mut fields = HashMap::new();
            for (key, value) in properties {
                fields.insert(key.clone(), compile_schema(value));
            }
            Generator::Object(ObjectGenerator::new(fields))
        }
    }
}
