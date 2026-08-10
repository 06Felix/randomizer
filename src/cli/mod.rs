mod args;
mod contract;
mod init;
mod lifecycle;
mod project;

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
    Adapter(#[from] crate::adapter::SpringBootAdapterError),
    #[error(transparent)]
    Mock(#[from] crate::mock::MockCompileError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error(transparent)]
    JavaDto(#[from] crate::dto::JavaDtoError),
    #[error(transparent)]
    JavaContractLock(#[from] crate::dto::JavaContractLockError),
    #[error(transparent)]
    Contract(#[from] contract::ContractError),
    #[error(transparent)]
    StandaloneServer(#[from] crate::server::ServerError),
    #[error("project is already initialized at {0}")]
    AlreadyInitialized(PathBuf),
    #[error("Randomizer is already running for this project; run `randomizer down` first")]
    AlreadyRunning,
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
    #[error("failed to start application command {program:?}: {source}")]
    ApplicationSpawn {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error("failed while waiting for application process: {0}")]
    ApplicationWait(#[source] io::Error),
    #[error("application process exited unsuccessfully with {0}")]
    ApplicationExit(std::process::ExitStatus),
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
}

pub async fn run_cli() -> Result<(), CliError> {
    let cli = Cli::parse();
    match cli.command {
        None | Some(args::Command::Serve) => {
            let config = Config::from_env()?;
            run(config).await?;
        }
        Some(args::Command::Init(args)) => init::init(args)?,
        Some(args::Command::Contract(args)) => contract::contract(args)?,
        Some(args::Command::Verify(args)) => project::verify(args)?,
        Some(args::Command::Up(args)) => lifecycle::up(args).await?,
        Some(args::Command::Dev(args)) => lifecycle::dev(args).await?,
        Some(args::Command::Status(args)) => lifecycle::status(args).await?,
        Some(args::Command::Inspect(args)) => project::inspect(args)?,
        Some(args::Command::Reset(args)) => project::reset(args).await?,
        Some(args::Command::Down(args)) => lifecycle::down(args).await?,
    }
    Ok(())
}
