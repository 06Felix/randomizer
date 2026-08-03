use rand::{Rng, RngExt};

#[derive(Debug)]
pub struct PrimitiveEnumGenerator {
    pub(crate) values: Vec<serde_json::Value>,
}

impl PrimitiveEnumGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let index = rng.random_range(0..self.values.len());
        self.values[index].clone()
    }
}
