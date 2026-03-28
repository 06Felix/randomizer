use rand::{Rng, RngExt};

/// Maximum decimal places applied when rounding generated floats for JSON output.
/// Larger values risk `powi` overflow and exceed [`f32`] meaningful precision.
const MAX_ROUNDING_DECIMAL_PLACES: i32 = 9;

/// Generates floating-point values within an inclusive range.
pub struct FloatGenerator {
    pub min: f32,
    pub max: f32,
    pub precision: u8,
}

impl FloatGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let value = rng.random_range(self.min..=self.max);
        let rounded = round_to_decimal_places(value, self.precision);
        serde_json::Value::Number(match serde_json::Number::from_f64(rounded) {
            Some(n) => n,
            None => serde_json::Number::from(0),
        })
    }
}

fn round_to_decimal_places(value: f32, precision: u8) -> f64 {
    let v = value as f64;
    let prec = (precision as i32).clamp(0, MAX_ROUNDING_DECIMAL_PLACES);
    if prec == 0 {
        return v.round();
    }
    let scale = 10_f64.powi(prec);
    (v * scale).round() / scale
}
