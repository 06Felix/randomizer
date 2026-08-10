use std::{ffi::OsString, path::PathBuf};

use clap::{Args, Parser, Subcommand};

use crate::dto::{BuildSystem, FieldPresence};

#[derive(Debug, Parser)]
#[command(
    name = "randomizer",
    version,
    about = "Schema-driven local HTTP API mocking"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize HTTP mocking in a project.
    Init(InitArgs),
    /// Import, refresh, or check generated response contracts.
    Contract(ContractArgs),
    /// Validate the manifest, contracts, fixtures, and mock routes.
    Verify(ProjectArgs),
    /// Start the local HTTP mocking harness.
    Up(ProjectArgs),
    /// Start the harness and run the application as a supervised child.
    Dev(DevArgs),
    /// Show gateway health.
    Status(ProjectArgs),
    /// List configured HTTP services and routes.
    Inspect(ProjectArgs),
    /// Clear mock scenarios and request history.
    Reset(ProjectArgs),
    /// Stop the managed harness for this project.
    Down(ProjectArgs),
    /// Run the original standalone generation server.
    Serve,
}

#[derive(Debug, Args)]
pub struct ContractArgs {
    #[command(subcommand)]
    pub command: ContractCommand,
}

#[derive(Debug, Subcommand)]
pub enum ContractCommand {
    /// Compile a Maven project and extract JSON Schema from a Java DTO.
    ImportJava(JavaContractArgs),
    /// Regenerate every Java-owned contract recorded in the lock file.
    Refresh(ProjectArgs),
    /// Fail when a generated Java contract is stale.
    Check(ProjectArgs),
}

#[derive(Debug, Args)]
pub struct JavaContractArgs {
    /// Stable contract name; used as the JSON filename and lock key.
    #[arg(long)]
    pub name: String,
    /// Fully-qualified Java type, including every generic argument.
    #[arg(long = "type")]
    pub root_type: String,
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
    #[arg(long, value_enum, default_value_t = BuildSystem::Auto)]
    pub build_system: BuildSystem,
    /// Whether all DTO properties or only annotated-required properties are required.
    #[arg(long, value_enum, default_value_t = FieldPresence::All)]
    pub field_presence: FieldPresence,
    /// Replace an existing contract that is not owned by the same Java type.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long, default_value = "spring-boot", value_parser = ["spring-boot"])]
    pub adapter: String,
    /// Do not add the generated configuration import to the application local profile.
    #[arg(long)]
    pub no_apply: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ProjectArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

#[derive(Debug, Args)]
#[command(trailing_var_arg = true)]
pub struct DevArgs {
    #[command(flatten)]
    pub project: ProjectArgs,
    #[arg(required = true, allow_hyphen_values = true)]
    pub application_command: Vec<OsString>,
}
