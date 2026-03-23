use std::collections::HashMap;

use crate::{compiler::compile_schema, generator::*, schema::Schema};

mod compiler;
mod generator;
mod schema;

fn main() {
    let mut rng = rand::rng();
    let schema = Schema::Int {
        min: None,
        max: None,
    };

    let int_generator = compile_schema(&schema);
    let float_generator = Generator::Float(FloatGenerator::new(Some(0.0), Some(100.0), Some(2)));
    let object_generator = Generator::Object(ObjectGenerator::new(HashMap::new()));

    let int_value = int_generator.generate(&mut rng);
    let float_value = float_generator.generate(&mut rng);
    let object_value = object_generator.generate(&mut rng);

    println!("Int: {}", int_value);
    println!("Float: {}", float_value);
    println!("Object: {:?}", object_value);
}
