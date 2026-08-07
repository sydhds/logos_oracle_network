mod args;
mod indexer;

use std::collections::HashSet;
use std::time::Duration;
// third-party
use clap::Parser as _;
use tokio::sync::watch;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
// internal
// use common::time_poller;
use common::time_info_poll;
use indexer::Indexer;
use crate::args::IndexerArgs;
use crate::indexer::channel_discover;

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

    let (watch_channel_ids_tx, watch_channel_ids_rx) = watch::channel(HashSet::new());
    let _channel_discover_handle = tokio::spawn(async move {
        channel_discover(Duration::from_millis(100), watch_channel_ids_tx).await
    });
    let (watch_time_info_tx, watch_time_info_rx) = watch::channel(None);
    let node_url = args.node_url.clone();
    let _watch_time_info_handle = tokio::spawn(async move  {
        time_info_poll(node_url, Duration::from_millis(100), watch_time_info_tx).await
    });

    // Wait for some initial channel_ids & time_info
    let mut chi = watch_channel_ids_rx.clone();
    let mut ti = watch_time_info_rx.clone();
    let (_first, _second) = tokio::join!(
        chi.wait_for(|state| !state.is_empty()),
        ti.wait_for(|state| state.is_some()),
    );

    // TODO: pass current_slot so indexer can wait 1 / 2 slots to start its work?
    let indexer = match Indexer::new(
        &args.node_url,
        args.node_auth_username,
        args.node_auth_password,
        watch_time_info_rx.clone(),
        watch_channel_ids_rx.clone(),
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