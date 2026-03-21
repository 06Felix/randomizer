pub struct ObjectGenerator {
    pub fields: std::collections::HashMap<String, crate::generator::Generator>,
}

impl ObjectGenerator {
    pub fn new(fields: std::collections::HashMap<String, crate::generator::Generator>) -> Self {
        Self { fields }
    }

    pub fn generate(&self, rng: &mut impl rand::Rng) -> serde_json::Value {
        let mut result = serde_json::Map::new();
        for (key, generator) in &self.fields {
            result.insert(key.clone(), generator.generate(rng));
        }
        serde_json::Value::Object(result)
    }
}