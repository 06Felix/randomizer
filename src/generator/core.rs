use crate::generator::composite::list::ListGenerator;
use crate::generator::composite::object::ObjectGenerator;
use crate::generator::primitives::enumeration::PrimitiveEnumGenerator;
use crate::generator::primitives::float::FloatGenerator;
use crate::generator::primitives::int::IntGenerator;
use crate::generator::primitives::string::StringGenerator;
use crate::generator::{BooleanGenerator, UUIDGenerator};

/// Runtime generator produced from a parsed schema.
#[derive(Debug)]
pub enum Generator {
    Int(IntGenerator),
    Float(FloatGenerator),
    String(StringGenerator),
    Enum(PrimitiveEnumGenerator),
    Object(ObjectGenerator),
    List(ListGenerator),
    Boolean(BooleanGenerator),
    Uuid(UUIDGenerator),
}

impl Generator {
    pub fn generate(&self, rng: &mut impl rand::Rng) -> serde_json::Value {
        match self {
            Generator::Int(int_gen) => int_gen.generate(rng),
            Generator::Float(float_gen) => float_gen.generate(rng),
            Generator::String(string_gen) => string_gen.generate(rng),
            Generator::Enum(enum_gen) => enum_gen.generate(rng),
            Generator::Object(object_gen) => object_gen.generate(rng),
            Generator::List(list_gen) => list_gen.generate(rng),
            Generator::Boolean(boolean_gen) => boolean_gen.generate(rng),
            Generator::Uuid(uuid_gen) => uuid_gen.generate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rand::{SeedableRng, rngs::SmallRng};
    use serde_json::json;

    use super::*;
    use crate::generator::{ListGenerator, ObjectGenerator, StringGenerator, StringGeneratorMode};

    fn rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    #[test]
    fn generates_integer_at_fixed_bound() {
        let value = Generator::Int(IntGenerator { min: 7, max: 7 }).generate(&mut rng());
        assert_eq!(value, json!(7));
    }

    #[test]
    fn generates_rounded_float() {
        let value = Generator::Float(FloatGenerator {
            min: 1.234,
            max: 1.234,
            precision: 2,
        })
        .generate(&mut rng());
        assert_eq!(value, json!(1.23));
    }

    #[test]
    fn generates_string_with_charset_and_wrappers() {
        let value = Generator::String(StringGenerator {
            prefix: "pre-".into(),
            suffix: "-post".into(),
            mode: StringGeneratorMode::Charset {
                min_length: 4,
                max_length: 4,
                charset: vec!['x'],
            },
        })
        .generate(&mut rng());
        assert_eq!(value, json!("pre-xxxx-post"));
    }

    #[test]
    fn generates_only_configured_enum_values() {
        let generator = Generator::Enum(PrimitiveEnumGenerator {
            values: vec![json!("a"), json!("b")],
        });
        let value = generator.generate(&mut rng());
        assert!([json!("a"), json!("b")].contains(&value));
    }

    #[test]
    fn boolean_boundary_probabilities_are_exact() {
        let mut rng = rng();
        let never = Generator::Boolean(BooleanGenerator {
            true_probability: 0,
        });
        let always = Generator::Boolean(BooleanGenerator {
            true_probability: 100,
        });

        for _ in 0..1_000 {
            assert_eq!(never.generate(&mut rng), json!(false));
            assert_eq!(always.generate(&mut rng), json!(true));
        }
    }

    #[test]
    fn generates_uuid_with_wrappers() {
        let value = Generator::Uuid(UUIDGenerator {
            prefix: "pre-".into(),
            suffix: "-post".into(),
        })
        .generate(&mut rng());
        let text = value.as_str().unwrap();
        let uuid = text
            .strip_prefix("pre-")
            .unwrap()
            .strip_suffix("-post")
            .unwrap();
        assert!(uuid::Uuid::parse_str(uuid).is_ok());
    }

    #[test]
    fn generates_list_from_item_generator() {
        let value = Generator::List(ListGenerator {
            min_length: 2,
            max_length: 2,
            item_generator: Box::new(Generator::Int(IntGenerator { min: 3, max: 3 })),
        })
        .generate(&mut rng());
        assert_eq!(value, json!([3, 3]));
    }

    #[test]
    fn generates_object_from_field_generators() {
        let value = Generator::Object(ObjectGenerator {
            fields: vec![(
                Arc::from("enabled"),
                Generator::Boolean(BooleanGenerator {
                    true_probability: 100,
                }),
            )],
        })
        .generate(&mut rng());
        assert_eq!(value, json!({"enabled": true}));
    }
}
