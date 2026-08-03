use rand::{Rng, RngExt};

/// Generates integers within an inclusive range.
#[derive(Debug)]
pub struct IntGenerator {
    pub(crate) min: i32,
    pub(crate) max: i32,
}

impl IntGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let value = rng.random_range(self.min..=self.max);
        serde_json::json!(value)
    }
}
