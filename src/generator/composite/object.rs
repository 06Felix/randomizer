use std::collections::HashMap;

use crate::generator::Generator;

/// Generates JSON objects by delegating generation to per-field generators.
pub struct ObjectGenerator {
    pub fields: HashMap<String, Generator>,
}

impl ObjectGenerator {
    pub fn generate(&self, rng: &mut impl rand::Rng) -> serde_json::Value {
        let mut result = serde_json::Map::new();
        for (key, generator) in &self.fields {
            result.insert(key.clone(), generator.generate(rng));
        }
        serde_json::Value::Object(result)
    }
}
