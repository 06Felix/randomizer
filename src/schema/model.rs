#![allow(dead_code)]
use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Schema {
    #[serde(rename = "int")]
    Int { min: Option<i32>, max: Option<i32> },
    #[serde(rename = "float")]
    Float {
        min: Option<f32>,
        max: Option<f32>,
        precision: Option<u8>,
    },
    #[serde(rename = "object")]
    Object { properties: HashMap<String, Schema> },
}
