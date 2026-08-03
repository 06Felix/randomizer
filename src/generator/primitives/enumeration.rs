use crate::generation::StableRng;

#[derive(Debug)]
pub struct PrimitiveEnumGenerator {
    pub(crate) values: Vec<serde_json::Value>,
}

impl PrimitiveEnumGenerator {
    pub(crate) fn generate(&self, rng: &mut StableRng) -> serde_json::Value {
        let index = rng.usize_inclusive(0, self.values.len() - 1);
        self.values[index].clone()
    }
}
