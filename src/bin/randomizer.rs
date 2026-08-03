use randomizer::{Config, run};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env()?;
    let filter = EnvFilter::try_new(&config.log_filter)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()?;
    run(config).await?;
    Ok(())
}
