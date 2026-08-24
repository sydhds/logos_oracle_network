mod sequencer;
mod pyth_fetch;
mod monitor;
mod zone_state;
mod args;
mod register_contract;

pub mod lon {
    include!(concat!(env!("OUT_DIR"), "/lon.rs"));
}

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::Arc;
// third-party
use crate::sequencer::Sequencer;
use anyhow::Context;
use clap::Parser;
use dashmap::DashMap;
// use futures::AsyncWriteExt;
use tokio::task::JoinSet;
use tracing::{
    info,
    debug,
    warn,
    error,
    level_filters::LevelFilter
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
// internal
use crate::args::SequencerArgs;
use crate::monitor::PriceMonitor;
use crate::pyth_fetch::fetch_price;
use common::time_info_poll;

#[tokio::main]
async fn main() {
    let args = SequencerArgs::parse();
    drop(run(args).await);
}

pub async fn run(args: SequencerArgs) -> anyhow::Result<()> {

    setup_tracing();

    info!("Starting oracle node sequencer...");
    debug!("args: {:?}", &args);

    // println!("Hello, world!");

    let pyth_base_url = "https://hermes.pyth.network/v2/updates/price/stream";
    let price_feed_eth_usdt = "ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace";

    let price_map = {
        let mut map = DashMap::new();
        map.insert(price_feed_eth_usdt.to_string(), vec![]);
        Arc::new(map)
    };

    let mut sequencer = Sequencer::new(
        &args.node_rest_url,
        args.data_folder.join(&args.oracle_key_path),
        args.data_folder.join(&args.key_path),
        args.node_auth_username,
        args.node_auth_password,
        args.data_folder.join(&args.checkpoint_path),
        price_map.clone(),
        price_feed_eth_usdt.to_string()
    ).context("Failed to initialize sequencer")?;

    // Setup queues
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    // let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel();

    // Setup price monitor
    let price_monitor = PriceMonitor::new(price_map.clone());

    // Setup time info poll
    let (time_info_tx, time_info_rx) = tokio::sync::watch::channel(None);
    let poll_interval = Duration::from_millis(20);

    let mut set = JoinSet::new();
    set.spawn(async move { time_info_poll( args.node_rest_url.clone(), poll_interval, time_info_tx).await } );
    set.spawn(async move { fetch_price(pyth_base_url, price_feed_eth_usdt, tx).await });
    set.spawn(async move { price_monitor.run(&mut rx).await });
    // FIXME: wait_ready ?
    set.spawn(async move { sequencer.run().await });

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Err(e)) => {
                error!("Task error: {:#?}", e);
                error!("Aborting...");
                break;
            },
            Ok(_) => {
                info!("Tasks finished");
                break;
            },
            Err(e) => {
                error!("Join error: {:#?}", e);
                break;
            },
        }
    }

    Ok(())
}

fn setup_tracing() {

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let console_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(console_layer)
        .with(filter)
        .init();
}
