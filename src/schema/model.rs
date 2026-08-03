use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::generation::GenerationOptions;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StringKind {
    Alphabetic,
    Numeric,
    Alphanumeric,
    Custom,
}

/// User-provided schema describing the shape of the random JSON output.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum Schema {
    #[serde(rename = "int")]
    Int { min: Option<i32>, max: Option<i32> },
    #[serde(rename = "float")]
    Float {
        min: Option<f32>,
        max: Option<f32>,
        precision: Option<u8>,
    },
    #[serde(rename = "string")]
    String {
        length: Option<usize>,
        min_length: Option<usize>,
        max_length: Option<usize>,
        prefix: Option<String>,
        suffix: Option<String>,
        string_type: StringKind,
        custom_charset: Option<String>,
    },
    #[serde(rename = "enum")]
    Enum { values: Vec<serde_json::Value> },
    #[serde(rename = "object")]
    Object { properties: HashMap<String, Schema> },
    #[serde(rename = "list")]
    List {
        length: Option<usize>,
        min_length: Option<usize>,
        max_length: Option<usize>,
        items: Box<Schema>,
    },
    #[serde(rename = "boolean")]
    Boolean { true_probability: i32 },
    #[serde(rename = "uuid")]
    Uuid {
        prefix: Option<String>,
        suffix: Option<String>,
    },
}

/// WebSocket request containing a schema and frequency.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WsRequest {
    pub schema: Schema,
    pub frequency: u64,
    pub seed: Option<u64>,
    pub sequence: Option<u64>,
    pub generator_version: Option<String>,
    pub contract_hash: Option<String>,
}

impl WsRequest {
    pub fn generation_options(&self) -> GenerationOptions {
        GenerationOptions {
            seed: self.seed,
            sequence: self.sequence,
            generator_version: self.generator_version.clone(),
            contract_hash: self.contract_hash.clone(),
        }
    }
}

/// REST request envelope for deterministic generation and replay.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateRequest {
    pub schema: Schema,
    pub seed: Option<u64>,
    pub sequence: Option<u64>,
    pub generator_version: Option<String>,
    pub contract_hash: Option<String>,
}

/// Accepts the deterministic envelope and the original raw-schema request for compatibility.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RestGenerateRequest {
    Envelope(GenerateRequest),
    Schema(Schema),
}

impl RestGenerateRequest {
    pub fn into_parts(self) -> (Schema, GenerationOptions) {
        match self {
            Self::Envelope(request) => (
                request.schema,
                GenerationOptions {
                    seed: request.seed,
                    sequence: request.sequence,
                    generator_version: request.generator_version,
                    contract_hash: request.contract_hash,
                },
            ),
            Self::Schema(schema) => (schema, GenerationOptions::default()),
        }
    }
}
