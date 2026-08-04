mod contract;
mod generate;

use crate::{
    error::{GenerationError, StandardContractError},
    generation::{
        ContractMetadata, GENERATOR_VERSION, GenerationContext, GenerationMode, GenerationOptions,
        GenerationResult, StableRng, generate_seed,
    },
    schema::JsonSchemaContract,
};

pub use contract::{ImportedContract, ValidationReport};

pub struct StandardGenerationPlan {
    contract: ImportedContract,
    seed: u64,
    generator_version: String,
    mode: GenerationMode,
}

impl StandardGenerationPlan {
    pub fn compile(
        contract: JsonSchemaContract,
        mode: GenerationMode,
        options: &GenerationOptions,
    ) -> Result<Self, GenerationError> {
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

        let contract = ImportedContract::import(contract)?;
        if let Some(provided) = &options.contract_hash
            && provided != &contract.metadata().content_hash
        {
            return Err(GenerationError::ContractHashMismatch {
                expected: contract.metadata().content_hash.clone(),
                provided: provided.clone(),
            });
        }

        Ok(Self {
            contract,
            seed: options.seed.unwrap_or_else(generate_seed),
            generator_version: generator_version.to_string(),
            mode,
        })
    }

    pub fn generate(&self, sequence: u64) -> Result<GenerationResult, GenerationError> {
        let context = GenerationContext {
            seed: self.seed,
            sequence,
            generator_version: self.generator_version.clone(),
            contract_hash: self.contract.metadata().content_hash.clone(),
        };
        let mut rng = StableRng::for_event(context.seed, context.sequence);
        let base_mode = if self.mode == GenerationMode::Invalid {
            GenerationMode::Valid
        } else {
            self.mode
        };
        let mut value = generate::generate(
            self.contract.schema(),
            self.contract.schema(),
            base_mode,
            &mut rng,
            "#",
            &mut Vec::new(),
        )?;

        let valid_report = self.contract.validate(&value);
        if !valid_report.valid {
            return Err(StandardContractError::GeneratedValueInvalid(
                valid_report
                    .violations
                    .iter()
                    .map(|violation| violation.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
            .into());
        }

        let violated_rule = if self.mode == GenerationMode::Invalid {
            let (invalid, violation) =
                generate::make_invalid(self.contract.schema(), &value, &self.contract)?;
            value = invalid;
            Some(violation)
        } else {
            None
        };

        Ok(GenerationResult {
            value,
            metadata: context,
            mode: self.mode,
            contract: Some(self.contract.metadata().clone()),
            violated_rule,
        })
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn contract_metadata(&self) -> &ContractMetadata {
        self.contract.metadata()
    }
}

pub fn generate_standard_value(
    contract: JsonSchemaContract,
    mode: GenerationMode,
    options: &GenerationOptions,
) -> Result<GenerationResult, GenerationError> {
    let sequence = options.sequence.unwrap_or(0);
    StandardGenerationPlan::compile(contract, mode, options)?.generate(sequence)
}

pub fn validate_standard_value(
    contract: JsonSchemaContract,
    value: &serde_json::Value,
) -> Result<ValidationReport, GenerationError> {
    Ok(ImportedContract::import(contract)?.validate(value))
}
