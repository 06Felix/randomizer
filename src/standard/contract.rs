use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::{GenerationError, StandardContractError},
    generation::{ContractMetadata, ViolatedRule, content_hash},
    schema::JsonSchemaContract,
};

pub struct ImportedContract {
    schema: Value,
    metadata: ContractMetadata,
    validator: jsonschema::Validator,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub valid: bool,
    pub violations: Vec<ViolatedRule>,
    pub contract: ContractMetadata,
}

impl ImportedContract {
    pub fn import(input: JsonSchemaContract) -> Result<Self, GenerationError> {
        validate_metadata("name", &input.name)?;
        validate_metadata("version", &input.version)?;
        validate_metadata("source", &input.source)?;
        validate_references(&input.schema)?;
        if let Some(dialect) = input.schema.get("$schema").and_then(Value::as_str)
            && dialect.trim_end_matches('#') != "https://json-schema.org/draft/2020-12/schema"
        {
            return Err(StandardContractError::UnsupportedDialect {
                dialect: dialect.to_string(),
            }
            .into());
        }

        if let Err(error) = jsonschema::draft202012::meta::validate(&input.schema) {
            return Err(StandardContractError::InvalidSchema(error.to_string()).into());
        }

        let calculated_hash = content_hash(&input.schema)?;
        if let Some(provided) = input.content_hash
            && provided != calculated_hash
        {
            return Err(GenerationError::ContractHashMismatch {
                expected: calculated_hash,
                provided,
            });
        }

        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&input.schema)
            .map_err(|error| StandardContractError::InvalidSchema(error.to_string()))?;

        Ok(Self {
            schema: input.schema,
            metadata: ContractMetadata {
                name: input.name,
                version: input.version,
                source: input.source,
                content_hash: calculated_hash,
            },
            validator,
        })
    }

    pub fn schema(&self) -> &Value {
        &self.schema
    }

    pub fn metadata(&self) -> &ContractMetadata {
        &self.metadata
    }

    pub fn validate(&self, value: &Value) -> ValidationReport {
        let mut violations: Vec<_> = self
            .validator
            .iter_errors(value)
            .map(|error| ViolatedRule {
                keyword: error.kind().keyword().to_string(),
                schema_path: error.schema_path().to_string(),
                instance_path: error.instance_path().to_string(),
                message: error.to_string(),
            })
            .collect();
        violations.sort_by(|left, right| {
            (&left.instance_path, &left.schema_path, &left.keyword).cmp(&(
                &right.instance_path,
                &right.schema_path,
                &right.keyword,
            ))
        });
        ValidationReport {
            valid: violations.is_empty(),
            violations,
            contract: self.metadata.clone(),
        }
    }
}

fn validate_metadata(field: &'static str, value: &str) -> Result<(), StandardContractError> {
    if value.trim().is_empty() {
        return Err(StandardContractError::EmptyMetadata { field });
    }
    Ok(())
}

fn validate_references(value: &Value) -> Result<(), StandardContractError> {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                && !reference.starts_with('#')
            {
                return Err(StandardContractError::ExternalReference {
                    reference: reference.to_string(),
                });
            }
            for nested in object.values() {
                validate_references(nested)?;
            }
        }
        Value::Array(values) => {
            for nested in values {
                validate_references(nested)?;
            }
        }
        _ => {}
    }
    Ok(())
}
