use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::generation::GenerationMode;

pub const CURRENT_MANIFEST_VERSION: u32 = 1;
pub type ManifestVersion = u32;

fn default_manifest_version() -> ManifestVersion {
    CURRENT_MANIFEST_VERSION
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    7263
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    #[serde(default = "default_manifest_version")]
    pub version: ManifestVersion,
    pub project: ProjectDefinition,
    #[serde(default)]
    pub services: Vec<ServiceDefinition>,
    #[serde(default)]
    pub routes: Vec<RouteDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectDefinition {
    pub name: String,
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub adapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceDefinition {
    pub id: String,
    #[serde(default)]
    pub config_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RouteDefinition {
    pub id: String,
    pub service: String,
    #[serde(rename = "match")]
    pub request_match: MatchDefinition,
    pub responses: Vec<ResponseDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct MatchDefinition {
    #[serde(default)]
    pub method: Option<String>,
    pub path: String,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResponseDefinition {
    #[serde(default = "default_status")]
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub delay_ms: u64,
    #[serde(default)]
    pub body: ResponseBodyDefinition,
    #[serde(default)]
    pub bindings: Vec<BindingDefinition>,
}

fn default_status() -> u16 {
    200
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResponseBodyDefinition {
    #[serde(default)]
    pub inline: Option<Value>,
    #[serde(default)]
    pub fixture: Option<String>,
    #[serde(default)]
    pub contract: Option<String>,
    #[serde(default)]
    pub mode: GenerationMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingDefinition {
    pub target: String,
    pub source: String,
}
