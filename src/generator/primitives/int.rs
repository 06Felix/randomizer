use crate::generation::StableRng;

/// Generates integers within an inclusive range.
#[derive(Debug)]
pub struct IntGenerator {
    pub(crate) min: i32,
    pub(crate) max: i32,
}

impl IntGenerator {
    pub(crate) fn generate(&self, rng: &mut StableRng) -> serde_json::Value {
        let value = rng.i32_inclusive(self.min, self.max);
        serde_json::json!(value)
    }
}
