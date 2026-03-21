use crate::generator::primitives::int::IntGenerator;
use crate::generator::primitives::float::FloatGenerator;
use crate::generator::composite::object::ObjectGenerator;

pub enum Generator {
    Int(IntGenerator),
    Float(FloatGenerator),
    Object(ObjectGenerator),
}

impl Generator {
    pub fn generate(&self, rng: &mut impl rand::Rng) -> serde_json::Value {
        match self {
            Generator::Int(int_gen) => int_gen.generate(rng),
            Generator::Float(float_gen) => float_gen.generate(rng),
            Generator::Object(object_gen) => object_gen.generate(rng),
        }
    }
}