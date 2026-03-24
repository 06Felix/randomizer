use rand::{Rng, RngExt};
use rust_decimal::{Decimal, prelude::ToPrimitive};

/// Generates floating-point values within an inclusive range.
pub struct FloatGenerator {
    pub min: f32,
    pub max: f32,
    pub precision: u8,
}

impl FloatGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let value = rng.random_range(self.min..=self.max);
        let decimal = Decimal::from_f32_retain(value)
            .unwrap()
            .round_dp(self.precision as u32);

        serde_json::Value::Number(serde_json::Number::from_f64(decimal.to_f64().unwrap()).unwrap())
    }
}
