mod sequencer;
mod pyth_fetch;
mod monitor;
mod zone_state;
mod args;
mod register_contract;
mod binance_fetch;
mod redstone_fetch;

pub mod lon {
    include!(concat!(env!("OUT_DIR"), "/lon.rs"));
}

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use std::sync::Arc;
// third-party
use crate::sequencer::Sequencer;
use anyhow::{anyhow, Context};
// use spel_framework::prelude::AccountId;
use clap::Parser;
use dashmap::DashMap;
use tokio::task::JoinSet;
use serde::Deserialize;
use tracing::{
    info,
    debug,
    // warn,
    error,
    level_filters::LevelFilter
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use url::Url;
// internal
use crate::args::SequencerArgs;
use crate::monitor::PriceMonitor;
// use crate::redstone_fetch::fetch_price;
use crate::register_contract::{sequencer_register, RegisterContractInfo};
use common::time_info_poll;

#[tokio::main]
async fn main() {
    let args = SequencerArgs::parse();
    if let Err(err) = run(args).await {
        error!("Error: {:#?}", err);
    }
}

pub async fn run(args: SequencerArgs) -> anyhow::Result<()> {

    setup_tracing();

    info!("Starting oracle node sequencer...");
    debug!("args: {:?}", &args);

    let cfg = parse_provider_config(args.provider_config.as_path())
        .context(format!("while parsing provider config: {}", args.provider_config.display()))?;
    
    // let provider = "binance";
    let provider = "redstone";
    let price_feed_normalized = "ETH/USD";
    let price_feed_url = cfg.endpoints.get(provider).ok_or(anyhow!("Cannot get an url for provider"))?;
    let price_feed_provider = cfg.feeds.get(provider)
        .ok_or(anyhow!("Cannot get feeds for provider"))?
        .get(price_feed_normalized)
        .ok_or(anyhow!("Cannot get price feed provider"))?
        .clone();

    let price_map = Arc::new(DashMap::new());

    let mut sequencer = Sequencer::new(
        &args.node_rest_url,
        args.data_folder.join(&args.oracle_key_path),
        args.data_folder.join(&args.key_path),
        args.node_auth_username,
        args.node_auth_password,
        args.data_folder.join(&args.checkpoint_path),
        price_map.clone(),
        price_feed_normalized.to_string()
    )
        .context("Failed to initialize sequencer")?;

    // Setup queues
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Setup price monitor
    let price_monitor = PriceMonitor::new(price_map.clone());

    // Setup time info poll
    let (time_info_tx, time_info_rx) = tokio::sync::watch::channel(None);
    let poll_interval = Duration::from_millis(20);

    // Register (or check it is already registered) to oracle_register contract
    {
        let file = std::fs::File::open(args.register_contract_config.as_path())
            .context(format!("Reading {}", args.register_contract_config.as_path().display()))?;
        let reader = std::io::BufReader::new(file);
        let cfg = serde_json::from_reader::<_, RegisterContractInfo>(reader)?;
        debug!("oracle register cfg: {:?}", cfg);
        sequencer_register(cfg).await?;
    }

    let mut set = JoinSet::new();
    set.spawn(async move { time_info_poll( args.node_rest_url.clone(), poll_interval, time_info_tx).await } );

    match provider {
        "binance" => {
            // Binance uses: "btcusdt"
            let url_str = format!("{}/{}@ticker", price_feed_url, price_feed_provider.to_lowercase());
            let url = Url::parse(&url_str).expect("Failed to parse Binance WS URL");
            set.spawn(async move {
                binance_fetch::fetch_price(
                    url,
                    price_feed_provider.as_str(),
                    price_feed_normalized,
                    tx
                ).await
            });
        },
        "redstone" => {
            // Can be: redstone, redstone-rapid, redstone-stocks, redstone-custom-urls
            let redstone_provider = "redstone";
            let url_str = format!("https://api.redstone.finance/prices?symbol={}&provider={}", price_feed_provider, redstone_provider);
            let url = Url::parse(&url_str).expect("Failed to parse Binance WS URL");
            set.spawn(async move {
                redstone_fetch::fetch_price(
                    url,
                    price_feed_provider.as_str(),
                    price_feed_normalized,
                    tx
                ).await
            });
        },
        "pyth" => {
            todo!()
        },
        _ => {
            unimplemented!()
        }
    }

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

type NormalizedFeed = String;
type ProviderName = String;
type ProviderFeed = String;

#[derive(Debug, Deserialize)]
pub struct PriceProviderConfig {
    pub endpoints: HashMap<String, String>, // TODO: ProviderName, Url
    pub feeds: HashMap<ProviderName, HashMap<NormalizedFeed, ProviderFeed>>,
}

fn parse_provider_config(json_cfg: &Path) -> anyhow::Result<PriceProviderConfig> {

    #[derive(Debug, Deserialize)]
    pub struct PriceProviderConfigRaw {
        pub endpoints: HashMap<String, String>,
        pub feeds: HashMap<NormalizedFeed, HashMap<ProviderName, ProviderFeed>>,
    }

    let json_reader = std::fs::File::open(json_cfg)?;
    let cfg_raw: PriceProviderConfigRaw = serde_json::from_reader(json_reader)?;

    let cfg = {

        let mut cfg_ = PriceProviderConfig {
            endpoints: cfg_raw.endpoints,
            feeds: HashMap::new(),
        };

        for (normalized_feed, providers) in cfg_raw.feeds {
            for (provider, provider_feed) in providers {
                // Skip empty string entries
                if !provider_feed.trim().is_empty() {
                    cfg_.feeds
                            .entry(provider)
                            .or_default()
                            .insert(normalized_feed.clone(), provider_feed);
                }
            }
        }

        cfg_

    };

    Ok(cfg)
}


