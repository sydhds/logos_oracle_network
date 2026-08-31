use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::Arc;
// third-party
use anyhow::Context;
use dashmap::DashMap;
use rand::Rng;
use url::Url;
use prost::Message;
use secp256k1::{Keypair, Secp256k1, XOnlyPublicKey};
use secp256k1::hashes::{sha256, Hash, sha256d};
use tracing::{info, debug, error};
// third-party - logos
use logos_blockchain_zone_sdk::{
    adapter::NodeHttpClient,
    sequencer::{
        Event, SequencerCheckpoint,
        SequencerClient,
        ZoneSequencer
    },
};
use lb_common_http_client::{BasicAuthCredentials, CommonHttpClient};
use lb_core::mantle::ops::channel::{
    ChannelId,
    inscribe::{Inscription, MAX_BYTES},
};
use lb_key_management_system_service::keys::{ED25519_SECRET_KEY_SIZE, Ed25519Key};
// internal
use crate::zone_state::InMemoryZoneState;
use common::{ParsedUpdate, PartialPriceObservation};
use crate::lon::PriceObservation;

pub struct Sequencer {
    sequencer: ZoneSequencer<NodeHttpClient>,
    client: SequencerClient,
    // handle: SequencerHandle<NodeHttpClient>,
    state: InMemoryZoneState,
    // pub queue_file: String,
    pub checkpoint_path: PathBuf,
    price_map: Arc<DashMap<String, Vec<PartialPriceObservation>>>,
    price_feed: String,
    // oracle pubk
    oracle_pubkey: Keypair,
}

impl Sequencer {

    pub(crate) fn new(
        node_endpoint: &str,
        oracle_key_path: PathBuf,
        signing_key_path: PathBuf,
        node_auth_username: Option<String>,
        node_auth_password: Option<String>,
        // queue_file: &str,
        checkpoint_path: PathBuf,
        // channel_path: &str,
        price_map: Arc<DashMap<String, Vec<PartialPriceObservation>>>,
        price_feed: String,
    ) -> anyhow::Result<Self> {

        let checkpoint = None;

        let oracle_pubkey = generate_oracle_credentials(oracle_key_path.as_path())?;

        let signing_key = load_or_create_signing_key(signing_key_path.as_path())?;
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
            checkpoint_path,
            price_map,
            price_feed,
            oracle_pubkey
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {

        info!("Starting sequencer...");

        let sequencer_client = self.client.clone();

        let price_map = self.price_map.clone();
        let price_feed = self.price_feed.clone();
        let keypair = self.oracle_pubkey.clone();
        let pubk = keypair.public_key();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_mins(1));

            // Wait for sequencer to be ready
            info!("Waiting for sequencer to be ready...");
            let mut ready_rx = sequencer_client.subscribe_ready();
            /*
            info!("Waiting for sequencer to be ready 2...");
            let _unused = tokio::time::timeout(
                Duration::from_secs(5),
                ready_rx.wait_for(|r| *r)
            )
                .await
                .inspect_err(|e| info!("e1: {}", e))?
                .inspect_err(|e| info!("e2: {}", e))?;
            // info!("unused: {:?}", _unused);
            drop(_unused);
            */

            info!("Sequencer ready, loop...");

            loop {

                // take prices from price_map
                let prices = if let Some(mut price_feed_ref_mut) = price_map.get_mut(&price_feed) {
                    std::mem::take(price_feed_ref_mut.value_mut())
                } else {
                    continue
                };

                // info!("price map: {:?}", price_map);
                let Some(price_latest) = prices.last() else {
                    // info!("No prices...");
                    // interval.tick().await;
                    continue
                };

                // store it
                // let Ok(prices_latest_json) = serde_json::to_string(price_latest) else { continue };
                // TODO: convert price_latest into PriceObservation
                info!("price_latest: {:?}", price_latest);
                /*
                let scaled_pyth_price = Price {
                    price: price_latest.price.price.parse::<i64>().unwrap(),
                    conf: price_latest.price.conf.parse::<u64>().unwrap(),
                    expo: price_latest.price.expo,
                    publish_time: price_latest.price.publish_time,
                }.scale_to_exponent(-6).expect("Price exceeds maximum representable bounds for target exponent");
                */

                let obs = {
                    let mut obs = PriceObservation {
                        feed_id: price_latest.feed_id.clone(),
                        price: price_latest.price,
                        decimals: price_latest.decimals,
                        round: 1045, // TODO: need Logos RPC doc
                        timestamp: price_latest.timestamp,
                        oracle_id: pubk.serialize().to_vec(),
                        signature: vec![],
                        membership_proof: vec![], // TODO: need LEZ register contract
                    };

                    let mut to_hash: Vec<u8> = vec![];
                    to_hash.extend(obs.feed_id.as_bytes());
                    to_hash.extend(obs.price.to_le_bytes().as_slice());
                    to_hash.extend(obs.decimals.to_le_bytes().as_slice());
                    to_hash.extend(obs.round.to_le_bytes().as_slice());
                    to_hash.extend(obs.timestamp.to_le_bytes().as_slice());
                    to_hash.extend(obs.oracle_id.clone());
                    let msg_hash = sha256d::Hash::hash(to_hash.as_slice());
                    let msg = secp256k1::Message::from_digest(msg_hash.to_byte_array());
                    // Generate the BIP-340 Schnorr Signature
                    let schnorr_sig = secp256k1::Secp256k1::new().sign_schnorr_no_aux_rand(&msg, &keypair);
                    obs.signature = schnorr_sig.serialize().to_vec();
                    obs
                };

                let payload_bytes = obs.encode_to_vec();
                debug!("payload bytes len: {}", payload_bytes.len());
                debug!("max bytes for inscription: {:?}", MAX_BYTES);

                let inscription = Inscription::try_from(payload_bytes)
                    .map_err(|e| SequencerError::InscriptionTooLarge(e.to_string()))
                    .unwrap();

                info!("Publishing...");
                let ts = std::time::Instant::now();
                if let Err(e) = sequencer_client.publish(inscription).await {
                    error!("failed to publish batch: {e}");
                } else {
                    let elapsed = ts.elapsed();
                    debug!("Submitted price update in {} milliseconds", elapsed.as_millis());
                }

                // Wait for 1 minutes between 2 prices update
                info!("Sequencer waiting...");
                interval.tick().await;
            }

            // Ok::<(), anyhow::Error>(())
        });

        loop {
            let event = self.sequencer.next_event().await;
            // println!("Handle event: {:?}", event);
            handle_event(event, &mut self.sequencer, &mut self.state, &self.checkpoint_path)?;
        }
    }

}

fn load_or_create_signing_key(path: &Path) -> anyhow::Result<Ed25519Key> {
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
        Ok(Ed25519Key::from_bytes(&key_array))
    } else {
        let mut key_bytes = [0u8; ED25519_SECRET_KEY_SIZE];
        let mut rng = rand::thread_rng();
        rng.fill(&mut key_bytes);
        println!("Start writing key file to: {}", path.display());
        fs::write(path, key_bytes)
            .context(format!("Error while writing key file to {}", path.display()))?
            // .expect("failed to write key file")
            ;
        Ok(Ed25519Key::from_bytes(&key_bytes))
    }
}

fn generate_oracle_credentials(_path: &Path) -> anyhow::Result<Keypair> {
    let secp = Secp256k1::new();
    let mut rng = rand::thread_rng();
    let keypair = Keypair::new(&secp, &mut rng);
    Ok(keypair)
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
    checkpoint_path: &Path,
) -> anyhow::Result<()> {
    match event {
        Event::Ready => {
            info!("Sequencer ready");
        },
        Event::BlocksProcessed { checkpoint, channel_update: _channel_update, finalized: _finalized } => {
            // println!("BlocksProcessed");
            save_checkpoint(checkpoint_path, &checkpoint)?;
        },
        Event::MempoolPending(_) | Event::TurnNotification { .. } => {}
    }

    Ok(())
}

fn save_checkpoint(path: &Path, checkpoint: &SequencerCheckpoint) -> anyhow::Result<()> {
    let data = serde_json::to_vec(checkpoint)
        .context(format!("Failed to serialize checkpoint: {:?}", checkpoint))?;
        // .expect("failed to serialize checkpoint");
    fs::write(path, data)
        .context(format!("Failed to write checkpoint to {}", path.display()))?;
        // .expect("failed to write checkpoint file");
    Ok(())
}