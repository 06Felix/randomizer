mod args;
mod contract;
mod init;
mod lifecycle;
mod project;
mod skill;
mod wiring;

use std::{io, net::SocketAddr, path::PathBuf};

use clap::Parser;
use thiserror::Error;

use crate::{Config, run};

pub use args::Cli;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Manifest(#[from] crate::project::ManifestError),
    #[error(transparent)]
    Skill(#[from] skill::SkillError),
    #[error(transparent)]
    Contract(#[from] crate::project::ContractError),
    #[error(transparent)]
    ContractReference(#[from] crate::project::ContractReferenceError),
    #[error(transparent)]
    Provider(#[from] crate::provider::ProviderError),
    #[error(transparent)]
    Wiring(#[from] crate::project::WiringError),
    #[error(transparent)]
    Mock(#[from] crate::mock::MockCompileError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error(transparent)]
    StandaloneServer(#[from] crate::server::ServerError),
    #[error("project is already initialized at {0}")]
    AlreadyInitialized(PathBuf),
    #[error("Randomizer is already running for this project; run `randomizer stop` first")]
    AlreadyRunning,
    #[error("failed to locate the Randomizer executable: {0}")]
    CurrentExecutable(#[source] io::Error),
    #[error("failed to start Randomizer in the background: {0}")]
    BackgroundStart(#[source] io::Error),
    #[error("Randomizer did not become ready within 5 seconds; see {log_path}")]
    StartTimeout { log_path: PathBuf },
    #[error("Randomizer exited before becoming ready with {status}; see {log_path}")]
    StartExit {
        status: std::process::ExitStatus,
        log_path: PathBuf,
    },
    #[error("refusing to stop process {pid} because it is not a managed Randomizer process")]
    StopOwnerMismatch { pid: u32 },
    #[error("Randomizer process {pid} did not stop within 5 seconds")]
    StopTimeout { pid: u32 },
    #[error("failed to encode YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("failed to encode JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid project host {host:?}: {source}")]
    InvalidHost {
        host: String,
        #[source]
        source: std::net::AddrParseError,
    },
    #[error("failed to bind project gateway to {address}: {source}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("project gateway failed: {0}")]
    Server(#[source] io::Error),
    #[error("project gateway task failed: {0}")]
    ServerTask(#[source] tokio::task::JoinError),
    #[error("failed to receive shutdown signal: {0}")]
    Signal(#[source] io::Error),
    #[error("failed to connect to Randomizer at {address}: {source}")]
    ResetConnect {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("Randomizer reset request failed: {0}")]
    ResetIo(#[source] io::Error),
    #[error("Randomizer rejected reset request: {0}")]
    ResetRejected(String),
    #[error("required managed-contract route {0:?} was not found in the manifest")]
    RequiredManagedContractRouteNotFound(String),
    #[error(
        "route {0:?} has no response backed by a managed contract in a generating mode; replace its inline, fixture, unmanaged, or mode: example body"
    )]
    RequiredManagedContractRouteNotGenerating(String),
}

pub async fn run_cli() -> Result<(), CliError> {
    let cli = Cli::parse();
    match cli.command {
        None | Some(args::Command::Serve) => {
            let config = Config::from_env()?;
            run(config).await?;
        }
        Some(args::Command::Init(args)) => init::init(args)?,
        Some(args::Command::Skill(args)) => skill::skill(args)?,
        Some(args::Command::Contract(args)) => contract::contract(args).await?,
        Some(args::Command::Wiring(args)) => wiring::wiring(args)?,
        Some(args::Command::Verify(args)) => project::verify(args)?,
        Some(args::Command::Start(args)) => lifecycle::start(args).await?,
        Some(args::Command::Stop(args)) => lifecycle::stop(args).await?,
        Some(args::Command::Status(args)) => lifecycle::status(args).await?,
        Some(args::Command::Inspect(args)) => project::inspect(args)?,
        Some(args::Command::Reset(args)) => project::reset(args).await?,
        Some(args::Command::RunProject(args)) => lifecycle::run_project(args).await?,
    }
    Ok(())
}
