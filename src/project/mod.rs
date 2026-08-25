mod contract_references;
mod contracts;
mod manifest;
mod paths;
mod validation;
mod wiring;

pub use contract_references::{ContractReferenceError, validate_managed_contract_references};
pub use contracts::{
    CONTRACT_LOCK_FORMAT_VERSION, ContractCheckReport, ContractError, ContractLock, ContractRecipe,
    ManagedContract, check_managed_contracts, load_contract_lock, managed_contract,
    save_managed_contract, save_managed_contract_if_unchanged,
};
pub use manifest::{
    BindingCoercion, BindingDefinition, CURRENT_MANIFEST_VERSION, ManifestVersion, MatchDefinition,
    ProjectDefinition, ProjectManifest, ResponseBodyDefinition, ResponseDefinition,
    RouteDefinition, ServiceBasePathBehavior, ServiceBaseSafety, ServiceDefinition,
    WiringDefinition, WiringFormat, WiringTarget,
};
pub use paths::ProjectPaths;
pub use validation::{ManifestError, validate_manifest};
pub use wiring::{WiringEntryReport, WiringError, WiringReport, apply_wiring, check_wiring};
