mod args;
mod indexer;

use std::time::Duration;
// use std::fmt::Debug;
use clap::Parser as _;
use indexer::Indexer;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use common::time_poller;
use crate::args::IndexerArgs;
use crate::indexer::spawn_channel_discoverer;

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

    info!("Oracle Indexer starting up...");
    info!("  Logos blockchain Node: {}", args.node_url);

    let watch_channel_ids = spawn_channel_discoverer(Duration::from_millis(100));
    let watch_time_info = time_poller(args.node_url.to_string(), Duration::from_millis(100));

    // Wait for some initial channel_ids & time_info
    let mut chi = watch_channel_ids.clone();
    let mut ti = watch_time_info.clone();
    let (_first, _second) = tokio::join!(
        chi.wait_for(|state| !state.is_empty()),
        ti.wait_for(|state| state.is_some()),
    );

    // TODO: pass current_slot so indexer can wait 1 / 2 slots to start its work?
    let indexer = match Indexer::new(
        &args.node_url,
        args.node_auth_username,
        args.node_auth_password,
        watch_time_info.clone(),
        watch_channel_ids.clone(),
    ) {
        Ok(i) => i,
        Err(e) => {
            error!("Indexer initialization failed: {e}");
            std::process::exit(1);
        }
    };

    info!("Launching indexer...");
    indexer.run().await;

    Ok(())
}