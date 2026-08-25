use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

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
    /// Install or update repository-local Randomizer agent skills.
    Skill(SkillArgs),
    /// Import, analyze, refresh, and verify response contracts.
    Contract(ContractArgs),
    /// Apply or verify application endpoint wiring.
    Wiring(WiringArgs),
    /// Validate the manifest, contracts, fixtures, and mock routes.
    Verify(ProjectArgs),
    /// Start the local HTTP mocking service.
    Start(StartArgs),
    /// Stop the managed local HTTP mocking service.
    Stop(ProjectArgs),
    /// Show gateway health.
    Status(ProjectArgs),
    /// List configured HTTP services and routes.
    Inspect(ProjectArgs),
    /// Clear mock scenarios and request history.
    Reset(ProjectArgs),
    /// Run the original standalone generation server.
    Serve,
    #[command(name = "__run-project", hide = true)]
    RunProject(ProjectArgs),
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Args)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillCommand {
    /// Synchronize the bundled Randomizer mock-management skill.
    Sync(SkillSyncArgs),
}

#[derive(Debug, Args)]
pub struct SkillSyncArgs {
    #[command(flatten)]
    pub project: ProjectArgs,
    /// Replace locally modified managed skill files.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ContractArgs {
    #[command(subcommand)]
    pub command: ContractCommand,
}

#[derive(Debug, Subcommand)]
pub enum ContractCommand {
    /// Import JSON Schema, an OpenAPI 3.1 response, or a serialized example.
    Import(ContractImportArgs),
    /// Analyze response metadata with an external provider.
    Analyze(ContractAnalyzeArgs),
    /// Re-run the import or provider recipe recorded for a managed contract.
    Refresh(ContractRefreshArgs),
    /// Verify contract artifacts and their source fingerprints.
    Check(ContractCheckArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ContractSourceFormat {
    JsonSchema,
    Openapi,
    SerializedExample,
}

#[derive(Debug, Args)]
pub struct ContractImportArgs {
    /// Stable contract name; also used as the artifact filename.
    pub name: String,
    #[command(flatten)]
    pub project: ProjectArgs,
    /// Project-relative JSON Schema, OpenAPI 3.1 document, or serialized JSON example.
    #[arg(long)]
    pub source: PathBuf,
    /// Source document type.
    #[arg(long, value_enum)]
    pub format: ContractSourceFormat,
    /// HTTP method associated with the response contract.
    #[arg(long)]
    pub method: String,
    /// HTTP endpoint path associated with the response contract.
    #[arg(long)]
    pub endpoint: String,
    /// HTTP response status associated with the response contract.
    #[arg(long, default_value_t = 200)]
    pub status: u16,
    /// Response media type when an OpenAPI operation defines more than one schema.
    #[arg(long)]
    pub media_type: Option<String>,
    /// Optional response root or wrapper symbol.
    #[arg(long)]
    pub root_symbol: Option<String>,
    /// User-facing contract version stored in the artifact.
    #[arg(long, default_value = "1")]
    pub contract_version: String,
}

#[derive(Debug, Args)]
pub struct ContractAnalyzeArgs {
    /// Stable contract name; also used as the artifact filename.
    pub name: String,
    #[command(flatten)]
    pub project: ProjectArgs,
    /// External provider executable implementing protocol version 1.
    #[arg(long)]
    pub provider: PathBuf,
    /// Argument passed to the provider executable; repeat as needed.
    #[arg(long = "provider-arg", allow_hyphen_values = true)]
    pub provider_args: Vec<String>,
    /// HTTP method being mocked.
    #[arg(long)]
    pub method: String,
    /// HTTP endpoint path being mocked.
    #[arg(long)]
    pub endpoint: String,
    /// HTTP response status being modeled.
    #[arg(long, default_value_t = 200)]
    pub status: u16,
    /// Expected response media type.
    #[arg(long)]
    pub media_type: Option<String>,
    /// Optional response root or wrapper symbol.
    #[arg(long)]
    pub root_symbol: Option<String>,
    /// Project-relative source inspected by the provider; repeat as needed.
    #[arg(long)]
    pub source: Vec<PathBuf>,
    /// User-facing contract version stored in the artifact.
    #[arg(long, default_value = "1")]
    pub contract_version: String,
    /// Maximum provider execution time in seconds.
    #[arg(long, default_value_t = 30)]
    pub timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub struct ContractRefreshArgs {
    pub name: String,
    #[command(flatten)]
    pub project: ProjectArgs,
    /// Maximum external provider execution time in seconds.
    #[arg(long, default_value_t = 30)]
    pub timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub struct ContractCheckArgs {
    /// Check one contract instead of every managed contract.
    pub name: Option<String>,
    #[command(flatten)]
    pub project: ProjectArgs,
}

#[derive(Debug, Args)]
pub struct WiringArgs {
    #[command(subcommand)]
    pub command: WiringCommand,
}

#[derive(Debug, Subcommand)]
pub enum WiringCommand {
    /// Apply configured local Randomizer URLs to application files.
    Apply(WiringProjectArgs),
    /// Verify configured application files already point to Randomizer.
    Check(WiringProjectArgs),
}

#[derive(Debug, Args)]
pub struct WiringProjectArgs {
    #[command(flatten)]
    pub project: ProjectArgs,
    /// Limit the operation to one configured service.
    #[arg(long)]
    pub service: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ProjectArgs {
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

#[derive(Debug, Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub project: ProjectArgs,
    /// Keep the service attached to this terminal and stream logs to standard output.
    #[arg(long)]
    pub foreground: bool,
}
