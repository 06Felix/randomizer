use crate::generation::StableRng;

/// Generates floating-point values within an inclusive range.
#[derive(Debug)]
pub struct FloatGenerator {
    pub(crate) min: f32,
    pub(crate) max: f32,
    pub(crate) precision: u8,
}

impl FloatGenerator {
    pub(crate) fn generate(&self, rng: &mut StableRng) -> serde_json::Value {
        let value = rng.f32_inclusive(self.min, self.max);
        let rounded = round_to_decimal_places(value, self.precision);
        serde_json::Value::Number(match serde_json::Number::from_f64(rounded) {
            Some(n) => n,
            None => serde_json::Number::from(0),
        })
    }
}

fn round_to_decimal_places(value: f32, precision: u8) -> f64 {
    let v = value as f64;
    let prec = i32::from(precision);
    if prec == 0 {
        return v.round();
    }
    let scale = 10_f64.powi(prec);
    (v * scale).round() / scale
}
