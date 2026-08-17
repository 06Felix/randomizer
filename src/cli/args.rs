use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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
