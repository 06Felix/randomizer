use rand::{Rng, RngExt};

pub struct IntGenerator {
    pub min: i32,
    pub max: i32,
}

impl IntGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let value = rng.random_range(self.min..=self.max);
        serde_json::json!(value)
    }
}

