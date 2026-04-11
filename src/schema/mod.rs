#![allow(unused_imports)]
pub mod model;
pub mod parser;

pub use model::{Schema, StringKind};
pub use parser::generate_schema_from_json_str;
