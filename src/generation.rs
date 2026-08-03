use rand::Rng;
use serde_json::Value;

use crate::{compiler::compile_schema, error::CompileError, schema::Schema};

/// Validates, compiles, and executes a schema once.
pub fn generate_value(schema: &Schema, rng: &mut impl Rng) -> Result<Value, CompileError> {
    Ok(compile_schema(schema)?.generate(rng))
}
