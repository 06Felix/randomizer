use rand::{Rng, RngExt};

/// Generates booleans using an integer percentage in the inclusive range 0..=100.
#[derive(Debug)]
pub struct BooleanGenerator {
    pub(crate) true_probability: u8,
}

impl BooleanGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        serde_json::json!(rng.random_range(0..100) < self.true_probability)
    }
}
