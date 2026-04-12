#![allow(dead_code)]
use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringKind {
    Alphabetic,
    Numeric,
    Alphanumeric,
    Custom,
}

/// User-provided schema describing the shape of the random JSON output.
#[derive(Debug, Deserialize)]
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
}
