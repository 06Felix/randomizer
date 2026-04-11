use rand::{Rng, RngExt};

use crate::generator::Generator;

pub struct ListGenerator {
    pub min_length: usize,
    pub max_length: usize,
    pub item_generator: Box<Generator>,
}

impl ListGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let length = rng.random_range(self.min_length..=self.max_length);
        let mut result = Vec::with_capacity(length);

        for _ in 0..length {
            result.push(self.item_generator.generate(rng));
        }

        serde_json::Value::Array(result)
    }
}
