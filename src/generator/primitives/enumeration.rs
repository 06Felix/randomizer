use rand::{Rng, RngExt};

pub struct PrimitiveEnumGenerator {
    pub values: Vec<serde_json::Value>,
}

impl PrimitiveEnumGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let index = rng.random_range(0..self.values.len());
        self.values[index].clone()
    }
}
