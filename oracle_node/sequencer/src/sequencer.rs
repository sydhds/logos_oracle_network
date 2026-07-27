use std::fs;
use std::path::Path;
use std::time::Duration;
// use anyhow::anyhow;
use dashmap::DashMap;
use rand::Rng;
// use tokio::sync::mpsc::UnboundedSender;
use url::Url;
use logos_blockchain_zone_sdk::{
    adapter::NodeHttpClient,
    sequencer::{
        Event, SequencerCheckpoint,
        // SequencerHandle,
        SequencerClient, ZoneSequencer
    },
};
use lb_common_http_client::{BasicAuthCredentials, CommonHttpClient};
// use lb_core::codec::SerializeOp;
use lb_core::mantle::ops::channel::{ChannelId, inscribe::Inscription};
use lb_core::mantle::ops::channel::inscribe::MAX_BYTES;
use lb_key_management_system_service::keys::{ED25519_SECRET_KEY_SIZE, Ed25519Key};
use crate::zone_state::InMemoryZoneState;
use common::{
    ParsedUpdate,
    // PriceInfo
};
use prost::Message;
use crate::lon::PriceObservation;

pub struct Sequencer {
    sequencer: ZoneSequencer<NodeHttpClient>,
    client: SequencerClient,
    // handle: SequencerHandle<NodeHttpClient>,
    state: InMemoryZoneState,
    // pub queue_file: String,
    pub checkpoint_path: String,
    price_map: DashMap<String, Vec<ParsedUpdate>>,
    price_feed: String,
}

impl Sequencer {

    pub(crate) fn new(
        node_endpoint: &str,
        signing_key_path: &str,
        node_auth_username: Option<String>,
        node_auth_password: Option<String>,
        // queue_file: &str,
        checkpoint_path: &str,
        // channel_path: &str,
        price_map: DashMap<String, Vec<ParsedUpdate>>,
        price_feed: String,
    ) -> anyhow::Result<Self> {

        let checkpoint = None;

        let signing_key = load_or_create_signing_key(Path::new(signing_key_path));
        let channel_id = ChannelId::from(signing_key.public_key().to_bytes());
        let node_url = Url::parse(node_endpoint)?; // .map_err(|e| anyhow!(e))?;
        let basic_auth = node_auth_username
            .map(|username| BasicAuthCredentials::new(username, node_auth_password));

        let node = NodeHttpClient::new(CommonHttpClient::new(basic_auth), node_url);
        let sequencer = ZoneSequencer::init(channel_id, signing_key, node, checkpoint);
        let client = sequencer.client();

        Ok(Self {
            sequencer,
            client,
            state: InMemoryZoneState::default(),
            // queue_file: queue_file.to_owned(),
            checkpoint_path: checkpoint_path.to_owned(),
            price_map,
            price_feed
        })
    }

    pub async fn run(&mut self) {
        
        let sequencer_client = self.client.clone();

        let price_map = self.price_map.clone();
        let price_feed = self.price_feed.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_mins(1));

            // Wait for sequencer to be ready
            let mut ready_rx = sequencer_client.subscribe_ready();
            let _ = ready_rx.wait_for(|r| *r).await;

            loop {

                // take prices from price_map
                let prices: Vec<ParsedUpdate> = std::mem::take(
                    price_map.get_mut(&price_feed).unwrap().value_mut()
                );
                let Some(price_latest) = prices.last() else { continue };

                // store it
                // let Ok(prices_latest_json) = serde_json::to_string(price_latest) else { continue };
                // TODO: convert price_latest into PriceObservation
                let obs = PriceObservation {
                    feed_id: "BTC/USDT".to_string(),
                    price: 65000_000_000,
                    decimals: 6,
                    round: 1045,
                    timestamp: 1718280000000,
                    oracle_id: vec![1, 2, 3, 4], // Your 32-byte pubkey
                    signature: vec![5, 6, 7],    // Your schnorr sig
                    membership_proof: vec![8, 9],
                };
                let payload_bytes = obs.encode_to_vec();
                println!("payload bytes len: {}", payload_bytes.len());
                println!("max bytes for inscription: {:?}", MAX_BYTES);

                let inscription = Inscription::try_from(payload_bytes)
                    .map_err(|e| SequencerError::InscriptionTooLarge(e.to_string()))
                    .unwrap();

                if let Err(e) = sequencer_client.publish(inscription).await {
                    eprintln!("failed to publish batch: {e}");
                } else {
                    println!("Submitted price update");
                }

                // Wait for 1 minutes between 2 prices update
                interval.tick().await;
            }
        });

        loop {
            let event = self.sequencer.next_event().await;
            println!("Handle event: {:?}", event);
            handle_event(event, &mut self.sequencer, &mut self.state, &self.checkpoint_path);
        }
    }

}

fn load_or_create_signing_key(path: &Path) -> Ed25519Key {
    if path.exists() {
        let key_bytes = fs::read(path).expect("failed to read key file");
        assert!(
            key_bytes.len() == ED25519_SECRET_KEY_SIZE,
            "invalid key file: expected {} bytes, got {}",
            ED25519_SECRET_KEY_SIZE,
            key_bytes.len()
        );
        let key_array: [u8; ED25519_SECRET_KEY_SIZE] =
            key_bytes.try_into().expect("length already checked");
        Ed25519Key::from_bytes(&key_array)
    } else {
        let mut key_bytes = [0u8; ED25519_SECRET_KEY_SIZE];
        let mut rng = rand::thread_rng();
        rng.fill(&mut key_bytes);
        fs::write(path, key_bytes).expect("failed to write key file");
        Ed25519Key::from_bytes(&key_bytes)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SequencerError {
    // #[error("URL parse error: {0}")]
    // Url(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Inscription too large: {0}")]
    InscriptionTooLarge(String),
}

fn handle_event(
    event: Event,
    _sequencer: &mut ZoneSequencer<NodeHttpClient>,
    _state: &mut InMemoryZoneState,
    checkpoint_path: &str,
) {
    match event {
        Event::Ready => {
            println!("Sequencer ready");
        },
        Event::BlocksProcessed { checkpoint, channel_update: _channel_update, finalized: _finalized } => {
            println!("BlocksProcessed");
            save_checkpoint(Path::new(checkpoint_path), &checkpoint);
        },
        Event::MempoolPending(_) | Event::TurnNotification { .. } => {}
    }
}

fn save_checkpoint(path: &Path, checkpoint: &SequencerCheckpoint) {
    let data = serde_json::to_vec(checkpoint).expect("failed to serialize checkpoint");
    fs::write(path, data).expect("failed to write checkpoint file");
}