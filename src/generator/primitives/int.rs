use rand::{Rng, RngExt};

pub struct IntGenerator {
    pub min: Option<i32>,
    pub max: Option<i32>,
}

impl IntGenerator {
    pub fn new(min: Option<i32>, max: Option<i32>) -> Self {
        Self { min, max }
    }

    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let value = rng.random_range(self.min.unwrap_or(0)..=self.max.unwrap_or(100));
        serde_json::json!(value)
    }
}