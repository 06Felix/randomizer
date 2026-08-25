use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::generation::GenerationMode;

/// Version 2 adds structured application wiring. Version 1 manifests remain valid when they do
/// not declare `services[].wiring`.
pub const CURRENT_MANIFEST_VERSION: u32 = 2;
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
    /// Accepted only to provide an actionable migration error for version 1 manifests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceDefinition {
    pub id: String,
    #[serde(default)]
    pub config_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wiring: Vec<WiringDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WiringDefinition {
    /// Exact project-relative application configuration file to update.
    pub file: String,
    pub format: WiringFormat,
    /// Environment/property key, JSON pointer, or YAML dot path selected by `format`.
    pub selector: String,
    pub target: WiringTarget,
    /// Required only when `target` is `route_url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Required only when `target` is `service_base_url`. This assertion makes the potentially
    /// broad effect of redirecting a shared service base URL explicit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_base_safety: Option<ServiceBaseSafety>,
    /// Required only when `target` is `service_base_url`. This assertion confirms that the
    /// application's HTTP client preserves the configured `/mock/<service-id>` base-path prefix
    /// when resolving endpoint paths, including paths written with a leading slash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_base_path_behavior: Option<ServiceBasePathBehavior>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WiringFormat {
    Dotenv,
    Properties,
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum WiringTarget {
    ServiceBaseUrl,
    RouteUrl,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ServiceBaseSafety {
    /// The selected configuration value is used only by the mocked integration.
    DedicatedSetting,
    /// Every call made through the selected service base URL is represented by a mock route.
    AllCallsMocked,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ServiceBasePathBehavior {
    /// The application HTTP client was inspected or tested and retains the configured base path.
    PreservesPrefix,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coerce: Option<BindingCoercion>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BindingCoercion {
    Integer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_language_neutral_v2_wiring_shape() {
        let manifest: ProjectManifest = serde_yaml::from_str(
            r#"
version: 2
project:
  name: example
services:
  - id: catalog
    wiring:
      - file: .env.local
        format: dotenv
        selector: CATALOG_BASE_URL
        target: service_base_url
        service_base_safety: dedicated_setting
        service_base_path_behavior: preserves_prefix
      - file: config.json
        format: json
        selector: /services/catalog/endpoint
        target: route_url
        route: get-catalog
routes:
  - id: get-catalog
    service: catalog
    match:
      method: GET
      path: /catalog
    responses:
      - body:
          inline: {}
"#,
        )
        .unwrap();

        assert_eq!(manifest.services[0].wiring.len(), 2);
        assert_eq!(manifest.services[0].wiring[0].format, WiringFormat::Dotenv);
        assert_eq!(
            manifest.services[0].wiring[0].service_base_safety,
            Some(ServiceBaseSafety::DedicatedSetting)
        );
        assert_eq!(
            manifest.services[0].wiring[0].service_base_path_behavior,
            Some(ServiceBasePathBehavior::PreservesPrefix)
        );
        assert_eq!(
            manifest.services[0].wiring[1].target,
            WiringTarget::RouteUrl
        );
        assert_eq!(
            manifest.services[0].wiring[1].route.as_deref(),
            Some("get-catalog")
        );

        let serialized = serde_yaml::to_string(&manifest).unwrap();
        assert!(serialized.contains("format: json"));
        assert!(serialized.contains("target: service_base_url"));
        assert!(serialized.contains("service_base_safety: dedicated_setting"));
        assert!(serialized.contains("service_base_path_behavior: preserves_prefix"));
    }

    #[test]
    fn parses_legacy_v1_manifest_without_wiring() {
        let manifest: ProjectManifest = serde_yaml::from_str(
            r#"
version: 1
project:
  name: legacy
services:
  - id: catalog
routes: []
"#,
        )
        .unwrap();

        assert_eq!(manifest.version, 1);
        assert!(manifest.services[0].wiring.is_empty());
    }

    #[test]
    fn parses_explicit_binding_coercion_without_changing_legacy_bindings() {
        let coerced: BindingDefinition = serde_yaml::from_str(
            r#"
target: /taskId
source: ${request.path.taskId}
coerce: integer
"#,
        )
        .unwrap();
        assert_eq!(coerced.coerce, Some(BindingCoercion::Integer));

        let legacy: BindingDefinition = serde_yaml::from_str(
            r#"
target: /taskId
source: ${request.path.taskId}
"#,
        )
        .unwrap();
        assert_eq!(legacy.coerce, None);
        assert!(!serde_yaml::to_string(&legacy).unwrap().contains("coerce:"));
    }
}
