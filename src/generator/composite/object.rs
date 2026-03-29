use std::sync::Arc;

use crate::generator::Generator;

/// Generates JSON objects by delegating generation to per-field generators.
pub struct ObjectGenerator {
    /// Sorted by field name for stable output. Keys are shared via `Arc` to keep compiled plans compact.
    pub fields: Vec<(Arc<str>, Generator)>,
}

impl ObjectGenerator {
    pub fn generate(&self, rng: &mut impl rand::Rng) -> serde_json::Value {
        let mut result = serde_json::Map::new();
        for (key, generator) in &self.fields {
            result.insert(key.as_ref().to_string(), generator.generate(rng));
        }
        serde_json::Value::Object(result)
    }
}
