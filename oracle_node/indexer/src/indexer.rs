use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::time::Duration;
// third-party
use futures::StreamExt as _;
use reqwest::Url;
use tracing::{error, info, warn};
use prost::Message;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use sha2::{Digest, Sha256};
// third-party - logos
use lb_common_http_client::{BasicAuthCredentials, CommonHttpClient};
use lb_core::mantle::ops::channel::ChannelId;
use logos_blockchain_zone_sdk::adapter::NodeHttpClient;
use logos_blockchain_zone_sdk::indexer::ZoneIndexer;
// internal
use common::{PricesContractInfo, RegisterContractInfo, TimeInfo};
use crate::indexer::lon::{AttestedPrice, PriceObservation};
use crate::prices_contract::publish_attested_price;
use crate::register_contract::fetch_registered;

pub mod lon {
    include!(concat!(env!("OUT_DIR"), "/lon.rs"));
}

pub struct Indexer {
    node_url: Url,
    node_auth_username: Option<String>,
    node_auth_password: Option<String>,
    time_rx: watch::Receiver<Option<TimeInfo>>,
    channels_rx: watch::Receiver<HashSet<ChannelId>>,
    active_workers: HashMap<ChannelId, JoinHandle<()>>,
    feed_workers: HashMap<String, mpsc::Sender<(ChannelId, PriceObservation)>>,
    pc_info: PricesContractInfo,
}

impl Indexer {

    pub fn new(
        node_endpoint: &str,
        node_auth_username: Option<String>,
        node_auth_password: Option<String>,
        time_rx: watch::Receiver<Option<TimeInfo>>,
        channels_rx: watch::Receiver<HashSet<ChannelId>>,
        pc_info: PricesContractInfo,
    ) -> anyhow::Result<Self> {
        let node_url = Url::parse(node_endpoint)?;
        Ok(Self {
            node_url,
            node_auth_username,
            node_auth_password,
            time_rx,
            channels_rx,
            active_workers: HashMap::new(),
            // TODO: maybe create feed workers for some known price id
            feed_workers: HashMap::new(),
            pc_info
        })
    }

    pub async fn run(mut self) {
        let (tx, mut rx) = mpsc::unbounded_channel::<(ChannelId, PriceObservation)>();
        let mut last_processed_slot = 0;
        let mut current_channels = HashSet::new();

        info!("Starting dynamic indexer coordinator...");

        loop {
            tokio::select! {
                // Listen for TimeInfo updates
                Ok(()) = self.time_rx.changed() => {

                    let current_slot = if let Some(info) = self.time_rx.borrow().as_ref() {
                        info.current_slot
                    } else {
                        continue;
                    };

                    if current_slot > last_processed_slot {
                        info!("New slot {}. Updating active channels...", current_slot);
                        last_processed_slot = current_slot;

                        // Instantly grab the latest polled channels without network I/O
                        let latest_channels = self.channels_rx.borrow().clone();

                        // News channels -> spawn tokio task (channel_worker)
                        for new_id in latest_channels.difference(&current_channels) {
                            info!("Spawning worker for new channel: {}", hex::encode(new_id.as_ref()));
                            // let handle = self.spawn_channel_worker(new_id.clone(), tx.clone());

                            let node_url = self.node_url.clone();
                            let node_auth_username = self.node_auth_username.clone();
                            let node_auth_password = self.node_auth_password.clone();
                            let tx = tx.clone();
                            let new_id = new_id.clone();
                            let handle = tokio::spawn(async move {
                                channel_worker(
                                    node_url,
                                    node_auth_username,
                                    node_auth_password,
                                    new_id,
                                    tx
                                ).await
                            });

                            self.active_workers.insert(new_id.clone(), handle);
                        }

                        // OLD channels -> abort tokio task
                        for old_id in current_channels.difference(&latest_channels) {
                            info!("Removing worker for deleted channel: {}", hex::encode(old_id.as_ref()));
                            if let Some(handle) = self.active_workers.remove(old_id) {
                                handle.abort();
                            }
                        }

                        // Update current list of channel ids
                        current_channels = latest_channels;
                    }
                },
                Some((channel_id, price_obs)) = rx.recv() => {

                    // Received channel_id & PriceObservation from workers
                    let hex_id = hex::encode(channel_id.as_ref());
                    println!("[Slot {} | Channel {}] Obs: {:?}", last_processed_slot, hex_id, price_obs);

                    let feed_id = price_obs.feed_id.clone();

                    // Get the queue to send our PriceObservation to the corresponding feed workers
                    let tx = match self.feed_workers.entry(feed_id.clone()) {
                        Entry::Occupied(entry) => entry.into_mut().clone(),
                        Entry::Vacant(entry) => {
                            info!("Spawning new processor for feed: {}", feed_id);
                            let feed_id = feed_id.clone();
                            let channels_rx = self.channels_rx.clone();
                            let time_info_rx = self.time_rx.clone();
                            let (worker_tx, worker_rx) = mpsc::channel(100);
                            let cfg = PriceFeedWorkerConfig {
                                feed_id, round_length: 1, quorum_threshold: 1 };
                            let pc_info = self.pc_info.clone();
                            let _worker_handle = tokio::spawn(async move {
                                price_feed_worker(cfg, channels_rx, worker_rx, time_info_rx, pc_info).await
                            });
                            entry.insert(worker_tx.clone());
                            worker_tx
                        }
                    };

                    // Now send our PriceObservation
                    if tx.send((channel_id.clone(), price_obs.clone())).await.is_err() {
                        // If send fails, the worker timed out and exited due to staleness.
                        // We need to spawn a fresh one and retry.
                        info!("Worker for {} was stale. Respawning.", feed_id);
                        let feed_id = feed_id.clone();
                        let channels_rx = self.channels_rx.clone();
                        let time_info_rx = self.time_rx.clone();
                        let (worker_tx, worker_rx) = mpsc::channel(100);
                        let cfg = PriceFeedWorkerConfig {
                            feed_id, round_length: 1, quorum_threshold: 1 };
                        let pc_info = self.pc_info.clone();
                        let _worker_handle = tokio::spawn(async move {
                            price_feed_worker(cfg, channels_rx, worker_rx, time_info_rx, pc_info).await
                        });
                        self.feed_workers.insert(price_obs.feed_id.clone(), tx.clone());
                        // Retry on the send on the newly created feed_worker
                        let _ = tx.send((channel_id, price_obs)).await; // TODO: report and log error here
                    }
                }
            }
        }
    }
}

/// PriceObservation worker (fetch from logos blockchain then send to price_feed_worker)
async fn channel_worker(
    node_url: Url,
    node_auth_username: Option<String>,
    node_auth_password: Option<String>,
    channel_id: ChannelId,
    tx: mpsc::UnboundedSender<(ChannelId, PriceObservation)>)
{
    // Build a ZoneIndexer (one per channel_id)
    let basic_auth = node_auth_username
        .map(|username| BasicAuthCredentials::new(username, node_auth_password));

    let common_client = CommonHttpClient::new(basic_auth);
    let node_client = NodeHttpClient::new(common_client, node_url);
    let zone_indexer = ZoneIndexer::new(channel_id.clone(), node_client);

    loop {
        let stream = match zone_indexer.follow().await {
            Ok(s) => s,
            Err(e) => {
                error!("Worker {} failed: {e}", hex::encode(channel_id.as_ref()));
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        futures::pin_mut!(stream);
        while let Some(zone_msg) = stream.next().await {
            let logos_blockchain_zone_sdk::ZoneMessage::Block(zone_block) = zone_msg else {
                continue;
            };

            let data = Vec::from(zone_block.data);
            if let Ok(obs) = PriceObservation::decode(data.as_slice()) {
                if tx.send((channel_id.clone(), obs)).is_err() {
                    return; // Coordinator shut down
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

}

/// Query SC for updated channel ids
pub async fn channel_discover(poll_interval: Duration, tx: watch::Sender<HashSet<ChannelId>>, rc_info: RegisterContractInfo) {

    loop {
        match fetch_registered(&rc_info).await {
            Ok(channels_vec) => {
                let channels_set: HashSet<ChannelId> = channels_vec
                    .into_iter()
                    .map(|channel_id| ChannelId::from(channel_id))
                    .collect();

                // Push the new state. Receivers can grab this instantly without blocking.
                if tx.send(channels_set).is_err() {
                    info!("Channel discoverer exiting: all receivers dropped.");
                    break;
                }
            }
            Err(e) => {
                error!("Failed to query channels from state: {}", e);
            }
        }
        tokio::time::sleep(poll_interval).await;
    }
}

/// A worker for a price feed
async fn price_feed_worker(
    cfg: PriceFeedWorkerConfig,
    channels_rx: watch::Receiver<HashSet<ChannelId>>,
    mut price_obs_rx: mpsc::Receiver<(ChannelId, PriceObservation)>,
    mut time_rx: watch::Receiver<Option<TimeInfo>>,
    pc_info: PricesContractInfo,
) {
    info!("Started feed worker for {}", cfg.feed_id);

    // Stale timeout
    let idle_timeout = Duration::from_mins(10);

    // State for the current round
    let mut current_round = 0;
    let mut current_round_observations: Vec<PriceObservation> = Vec::new();

    // Initialize current_round based on the current slot
    if let Some(info) = time_rx.borrow().as_ref() {
        current_round = info.current_slot / cfg.round_length;
    }

    loop {

        tokio::select! {

            Ok(()) = time_rx.changed() => {
                let current_slot = if let Some(info) = time_rx.borrow().as_ref() {
                    info.current_slot
                } else {
                    continue;
                };

                let active_round = current_slot / cfg.round_length;

                if active_round > current_round {

                    // new round -> compute PriceAttestation

                    let obs_count = current_round_observations.len();

                    if obs_count >= cfg.quorum_threshold {
                        // Extract just the integer prices for median calculation
                        let prices: Vec<i64> = current_round_observations
                            .iter()
                            .map(|obs| obs.price)
                            .collect();

                        let attested_median = compute_median(prices);

                        // Assuming all valid obs have the same decimals, grab from the first
                        // TODO: filter if some the required decimals
                        let decimals = current_round_observations
                            .first()
                            .map(|o| o.decimals)
                            .unwrap_or(6); // TODO: no hardcoded value

                        let feed_id: [u8; 32] = {
                            let r = Sha256::digest(cfg.feed_id.clone());
                            r.into()
                        };

                        let attested_price = AttestedPrice {
                            feed_id: feed_id.to_vec(),
                            price: attested_median,
                            decimals,
                            valid_count: obs_count as u32,
                            round: current_round as i64,
                            confidence: 0, // TODO: Implement 1.4826 * MAD
                        };

                        info!("[Feed {}] Attested round {}: Price {}, Count {}",
                              cfg.feed_id, current_round, attested_price.price, obs_count);

                        if let Err(e) = publish_attested_price(&pc_info, attested_price).await {
                            error!("Error while publishing attested price: {}", e);
                        }

                    } else {
                        warn!("[Feed {}] Round {} missed quorum ({} < {})",
                              cfg.feed_id, current_round, obs_count, cfg.quorum_threshold);
                    }

                    // Reset state for the new round
                    current_round_observations.clear();
                    current_round = active_round;
                }
            },
            // Wait for a message, but timeout if idle too long
            msg = timeout(idle_timeout, price_obs_rx.recv()) => {
                match msg {
                    Ok(Some((channel_id, obs))) => {
                        // FILTER: Ensure the channel is STILL active.
                        // This prevents processing messages that were buffered in the channel
                        // just before the ZoneIndexer worker was aborted on a slot change.
                        if !channels_rx.borrow().contains(&channel_id) {
                            tracing::debug!("Discarding worker from inactive channel: {}", hex::encode(channel_id.as_ref()));
                            continue;
                        }

                        println!("[Feed {}] processing valid observation: {}", cfg.feed_id, obs.price);
                        // TODO: mean computing...

                    }
                    Ok(None) => {
                        warn!("Main indexer loop dropped the sender, time to exit...");
                        break;
                    }
                    Err(_) => {
                        // Timeout handling
                        info!("Feed worker for {} timed out due to inactivity. Shutting down.", cfg.feed_id);
                        break;
                    }
                }
            }
        }
    }
}

struct PriceFeedWorkerConfig {
    feed_id: String,
    round_length: u64, // e.g. 1 for 1 block per round
    quorum_threshold: usize, // e.g. N active oracles required
}

fn compute_median(mut prices: Vec<i64>) -> i64 {
    if prices.is_empty() {
        return 0;
    }
    prices.sort_unstable();
    let mid = prices.len() / 2;
    if prices.len() % 2 == 0 {
        // Average the two middle values if even
        (prices[mid - 1] + prices[mid]) / 2
    } else {
        prices[mid]
    }
}

