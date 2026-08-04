pub mod rest;
pub mod websocket;

pub use rest::{generate, validate_contract};
pub use websocket::stream;
