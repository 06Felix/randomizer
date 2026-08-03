use proptest::prelude::*;
use randomizer::{
    compiler::compile_schema,
    generation::{GENERATOR_VERSION, GenerationContext},
    schema::Schema,
};

fn context(sequence: u64) -> GenerationContext {
    GenerationContext {
        seed: 42,
        sequence,
        generator_version: GENERATOR_VERSION.to_string(),
        contract_hash: "property-test".to_string(),
    }
}

proptest! {
    #[test]
    fn generated_integers_stay_inside_inclusive_bounds(
        min in any::<i32>(),
        width in 0_u16..=1_000,
    ) {
        let max = min.saturating_add(i32::from(width));
        let generator = compile_schema(&Schema::Int {
            min: Some(min),
            max: Some(max),
        }).unwrap();
        for sequence in 0..100 {
            let value = generator.generate(&context(sequence)).as_i64().unwrap();
            prop_assert!((i64::from(min)..=i64::from(max)).contains(&value));
        }
    }

    #[test]
    fn generated_list_lengths_stay_inside_bounds(
        min in 0_usize..=50,
        extra in 0_usize..=50,
    ) {
        let max = (min + extra).min(100);
        let generator = compile_schema(&Schema::List {
            length: None,
            min_length: Some(min),
            max_length: Some(max),
            items: Box::new(Schema::Boolean { true_probability: 50 }),
        }).unwrap();
        for sequence in 0..20 {
            let length = generator.generate(&context(sequence)).as_array().unwrap().len();
            prop_assert!((min..=max).contains(&length));
        }
    }
}
