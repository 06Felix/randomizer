use crate::{generation::StableRng, generator::Generator};

#[derive(Debug)]
pub struct ListGenerator {
    pub(crate) min_length: usize,
    pub(crate) max_length: usize,
    pub(crate) item_generator: Box<Generator>,
}

impl ListGenerator {
    pub(crate) fn generate(&self, rng: &mut StableRng) -> serde_json::Value {
        let length = rng.usize_inclusive(self.min_length, self.max_length);
        let mut result = Vec::with_capacity(length);

        for _ in 0..length {
            result.push(self.item_generator.generate_with_rng(rng));
        }

        serde_json::Value::Array(result)
    }
}
