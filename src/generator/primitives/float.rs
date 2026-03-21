use rand::{Rng, RngExt};

pub struct FloatGenerator {
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub precision: Option<u32>,
}

impl FloatGenerator {
    pub fn new(min: Option<f32>, max: Option<f32>, precision: Option<u32>) -> Self {
        Self { min, max, precision }
    }

    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let value = rng.random_range(self.min.unwrap_or(0.0)..=self.max.unwrap_or(1.0));
        let precision = self.precision.unwrap_or(2);
        let value = format!("{:.1$}", value, precision as usize);
        serde_json::json!(value)
    }
}