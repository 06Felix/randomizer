use crate::generation::StableRng;

/// Generates booleans using an integer percentage in the inclusive range 0..=100.
#[derive(Debug)]
pub struct BooleanGenerator {
    pub(crate) true_probability: u8,
}

impl BooleanGenerator {
    pub(crate) fn generate(&self, rng: &mut StableRng) -> serde_json::Value {
        serde_json::json!(rng.below(100) < u64::from(self.true_probability))
    }
}
