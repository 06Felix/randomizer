use rand::{Rng, RngExt};

/// Generates integers within an inclusive range.
pub struct BooleanGenerator {
    pub true_probability: i32,
}

impl BooleanGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let true_probability = self.true_probability.clamp(0, 100);
        serde_json::json!(rng.random_range(0..=100) < true_probability)
    }
}
