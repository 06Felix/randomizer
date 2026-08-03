use rand::RngExt;
use serde::{Deserialize, Serialize};

/// Version of the complete deterministic generation behavior.
///
/// This must be incremented whenever RNG derivation or value generation semantics change.
pub const GENERATOR_VERSION: &str = "1";

/// Largest exactly representable integer in JavaScript, keeping generated seeds portable in JSON.
const MAX_PORTABLE_SEED: u64 = (1_u64 << 53) - 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationContext {
    pub seed: u64,
    pub sequence: u64,
    pub generator_version: String,
    pub contract_hash: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct GenerationOptions {
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub sequence: Option<u64>,
    #[serde(default)]
    pub generator_version: Option<String>,
    #[serde(default)]
    pub contract_hash: Option<String>,
}

/// The returned metadata is the exact context needed to replay an event.
pub type GenerationMetadata = GenerationContext;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationResult {
    pub value: serde_json::Value,
    pub metadata: GenerationMetadata,
}

pub fn generate_seed() -> u64 {
    rand::rng().random::<u64>() & MAX_PORTABLE_SEED
}
