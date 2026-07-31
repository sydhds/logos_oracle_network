use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::time::Duration;
use futures::StreamExt as _;
use lb_common_http_client::{BasicAuthCredentials, CommonHttpClient};
use lb_core::mantle::ops::channel::ChannelId;
use logos_blockchain_zone_sdk::adapter::NodeHttpClient;
use logos_blockchain_zone_sdk::indexer::ZoneIndexer;
use reqwest::Url;
use tracing::{error, info, warn};
use common::TimeInfo;
use crate::indexer::lon::PriceObservation;
use prost::Message;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::timeout;

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
}

impl Indexer {

    pub fn new(
        node_endpoint: &str,
        node_auth_username: Option<String>,
        node_auth_password: Option<String>,
        time_rx: watch::Receiver<Option<TimeInfo>>,
        channels_rx: watch::Receiver<HashSet<ChannelId>>,
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

                        // News channels -> spawn tokio task
                        for new_id in latest_channels.difference(&current_channels) {
                            info!("Spawning worker for new channel: {}", hex::encode(new_id.as_ref()));
                            let handle = self.spawn_channel_worker(new_id.clone(), tx.clone());
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
                    let mut tx = match self.feed_workers.entry(feed_id.clone()) {
                        Entry::Occupied(entry) => entry.into_mut().clone(),
                        Entry::Vacant(entry) => {
                            info!("Spawning new processor for feed: {}", feed_id);
                            let new_tx = spawn_feed_worker(feed_id.clone(), self.channels_rx.clone());
                            entry.insert(new_tx.clone());
                            new_tx
                        }
                    };

                    // Now send our PriceObservation
                    if tx.send((channel_id.clone(), price_obs.clone())).await.is_err() {
                        // If send fails, the worker timed out and exited due to staleness.
                        // We need to spawn a fresh one and retry.
                        info!("Worker for {} was stale. Respawning.", feed_id);
                        tx = spawn_feed_worker(feed_id.clone(), self.channels_rx.clone());
                        self.feed_workers.insert(feed_id, tx.clone());
                        // Retry on the send on the newly created feed_worker
                        let _ = tx.send((channel_id, price_obs)).await; // TODO: report and log error here
                    }
                }
            }
        }
    }

    fn spawn_channel_worker(
        &self,
        channel_id: ChannelId,
        tx: mpsc::UnboundedSender<(ChannelId, PriceObservation)>
    ) -> JoinHandle<()> {

        // Build a ZoneIndexer (one per channel_id)
        let basic_auth = self.node_auth_username.clone()
            .map(|username| BasicAuthCredentials::new(username, self.node_auth_password.clone()));
        let common_client = CommonHttpClient::new(basic_auth);
        let node_client = NodeHttpClient::new(common_client, self.node_url.clone());
        let zone_indexer = ZoneIndexer::new(channel_id.clone(), node_client);

        tokio::spawn(async move {
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
        })
    }

}

pub fn spawn_channel_discoverer(
    poll_interval: Duration,
) -> watch::Receiver<HashSet<ChannelId>> {
    let (tx, rx) = watch::channel(HashSet::new());

    tokio::spawn(async move {
        loop {
            // MOCK: Replace with your actual smart contract/state query
            match mock_query_contract_for_channels().await {
                Ok(channels_vec) => {
                    let channels_set: HashSet<ChannelId> = channels_vec.into_iter().collect();

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
    });

    rx
}

fn spawn_feed_worker(
    feed_id: String,
    channels_rx: watch::Receiver<HashSet<ChannelId>>,
) -> mpsc::Sender<(ChannelId, PriceObservation)> {
    // Buffer up to 100 observations per feed
    let (tx, mut rx) = mpsc::channel::<(ChannelId, PriceObservation)>(100);

    tokio::spawn(async move {
        info!("Started feed worker for {}", feed_id);

        // Stale timeout: 10 minutes
        let idle_timeout = Duration::from_secs(600);

        loop {
            // Wait for a message, but timeout if idle too long
            match timeout(idle_timeout, rx.recv()).await {
                Ok(Some((channel_id, obs))) => {
                    // FILTER: Ensure the channel is STILL active.
                    // This prevents processing messages that were buffered in the channel
                    // just before the ZoneIndexer worker was aborted on a slot change.
                    if !channels_rx.borrow().contains(&channel_id) {
                        tracing::debug!("Discarding obs from inactive channel: {}", hex::encode(channel_id.as_ref()));
                        continue;
                    }

                    println!("[Feed {}] processing valid observation: {}", feed_id, obs.price);
                    // TODO: mean computing...

                }
                Ok(None) => {
                    warn!("Main indexer loop dropped the sender, time to exit...");
                    break;
                }
                Err(_) => {
                    // Timeout handling
                    info!("Feed worker for {} timed out due to inactivity. Shutting down.", feed_id);
                    break;
                }
            }
        }
    });

    tx
}

async fn mock_query_contract_for_channels() -> anyhow::Result<Vec<ChannelId>> {
    // TODO
    Ok(vec![])
}
