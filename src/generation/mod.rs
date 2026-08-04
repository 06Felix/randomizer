mod canonical;
mod context;
mod stable_rng;

use crate::{
    compiler::compile_schema, error::GenerationError, generator::Generator, schema::Schema,
};

pub use canonical::{canonical_json, canonical_schema_json, content_hash, contract_hash};
pub use context::{
    ContractMetadata, GENERATOR_VERSION, GenerationContext, GenerationMetadata, GenerationMode,
    GenerationOptions, GenerationResult, ViolatedRule, generate_seed,
};
pub(crate) use stable_rng::StableRng;

/// A validated and compiled contract that can generate independently replayable events.
#[derive(Debug)]
pub struct GenerationPlan {
    generator: Generator,
    seed: u64,
    generator_version: String,
    contract_hash: String,
}

impl GenerationPlan {
    pub fn compile(schema: &Schema, options: &GenerationOptions) -> Result<Self, GenerationError> {
        let generator_version = options
            .generator_version
            .as_deref()
            .unwrap_or(GENERATOR_VERSION);
        if generator_version != GENERATOR_VERSION {
            return Err(GenerationError::UnsupportedGeneratorVersion {
                provided: generator_version.to_string(),
                supported: GENERATOR_VERSION,
            });
        }

        let calculated_hash = contract_hash(schema)?;
        if let Some(provided_hash) = &options.contract_hash
            && provided_hash != &calculated_hash
        {
            return Err(GenerationError::ContractHashMismatch {
                expected: calculated_hash,
                provided: provided_hash.clone(),
            });
        }

        Ok(Self {
            generator: compile_schema(schema)?,
            seed: options.seed.unwrap_or_else(generate_seed),
            generator_version: generator_version.to_string(),
            contract_hash: calculated_hash,
        })
    }

    pub fn generate(&self, sequence: u64) -> GenerationResult {
        let context = GenerationContext {
            seed: self.seed,
            sequence,
            generator_version: self.generator_version.clone(),
            contract_hash: self.contract_hash.clone(),
        };
        let value = self.generator.generate(&context);
        GenerationResult {
            value,
            metadata: context,
            mode: GenerationMode::Valid,
            contract: None,
            violated_rule: None,
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn contract_hash(&self) -> &str {
        &self.contract_hash
    }
}

/// Validates, compiles, and executes a schema once. Supplying the returned metadata replays it.
pub fn generate_value(
    schema: &Schema,
    options: &GenerationOptions,
) -> Result<GenerationResult, GenerationError> {
    let sequence = options.sequence.unwrap_or(0);
    Ok(GenerationPlan::compile(schema, options)?.generate(sequence))
}
