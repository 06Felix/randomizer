use randomizer::cli::run_cli;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    if let Err(error) = try_main().await {
        eprintln!("error: {error}");
        let mut source = error.source();
        while let Some(cause) = source {
            eprintln!("  caused by: {cause}");
            source = cause.source();
        }
        std::process::exit(1);
    }
}

async fn try_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = EnvFilter::try_new(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()?;
    run_cli().await?;
    Ok(())
}
