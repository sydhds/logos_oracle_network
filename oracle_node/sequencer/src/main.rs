mod sequencer;
mod pyth_fetch;
mod monitor;
mod zone_state;
mod args;
mod time_fetch;

pub mod lon {
    include!(concat!(env!("OUT_DIR"), "/lon.rs"));
}

use anyhow::Context;
use clap::Parser;
use dashmap::DashMap;
// use futures::AsyncWriteExt;
use tokio::task::JoinSet;
use crate::args::SequencerArgs;
use crate::monitor::PriceMonitor;
use crate::pyth_fetch::fetch_price;
use crate::sequencer::Sequencer;

#[tokio::main]
async fn main() {
    let args = SequencerArgs::parse();
    drop(run(args).await);
}

pub async fn run(args: SequencerArgs) -> anyhow::Result<()> {

    // println!("Hello, world!");

    let pyth_base_url = "https://hermes.pyth.network/v2/updates/price/stream";
    let price_feed_eth_usdt = "ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace";

    let price_map = DashMap::new();
    price_map.insert(price_feed_eth_usdt.to_string(), vec![]);

    let mut sequencer = Sequencer::new(
        &args.node_url,
        &args.oracle_key_path,
        &args.key_path,
        args.node_auth_username,
        args.node_auth_password,
        &args.checkpoint_path,
        price_map.clone(),
        price_feed_eth_usdt.to_string()
    ).context("Failed to initialize sequencer")?;

    // Setup queues
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    // let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel();

    // Setup price monitor
    let price_monitor = PriceMonitor::new(price_map.clone());

    let mut set = JoinSet::new();
    set.spawn(async move { sequencer.run().await });
    set.spawn(async move { fetch_price(pyth_base_url, price_feed_eth_usdt, tx).await });
    set.spawn(async move { price_monitor.run(&mut rx).await });

    while let Some(res) = set.join_next().await {
        match res {
            Ok(_) => {
                println!("Task finished");
                break;
            },
            Err(e) => {
                println!("Error: {:#?}", e);
                break;
            },
        }
    }

    Ok(())
}