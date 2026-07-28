mod args;
mod indexer;

// use std::fmt::Debug;
use clap::Parser as _;
use indexer::Indexer;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use crate::args::IndexerArgs;

#[tokio::main]
async fn main() {
    let args = IndexerArgs::parse();
    drop(run(args).await);
}

pub async fn run(args: IndexerArgs) -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Sqlite Indexer starting up...");
    info!("  Logos blockchain Node: {}", args.node_url);

    let indexer = match Indexer::new(
        &args.node_url,
        &args.channel_path,
        args.node_auth_username,
        args.node_auth_password,
    ) {
        Ok(i) => i,
        Err(e) => {
            error!("Indexer initialization failed: {e}");
            std::process::exit(1);
        }
    };
    info!("Indexer ready...");

    info!("Launching indexer...");
    indexer.run().await;

    Ok(())
}