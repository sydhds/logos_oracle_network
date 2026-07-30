use std::fs;
use anyhow::{anyhow, Context};
use futures::StreamExt as _;
use lb_common_http_client::{BasicAuthCredentials, CommonHttpClient};
use lb_core::mantle::ops::channel::ChannelId;
use logos_blockchain_zone_sdk::adapter::NodeHttpClient;
use logos_blockchain_zone_sdk::indexer::ZoneIndexer;
use reqwest::Url;
use tracing::{error, info};
use common::ParsedUpdate;
use crate::indexer::lon::PriceObservation;
use prost::Message;

pub mod lon {
    include!(concat!(env!("OUT_DIR"), "/lon.rs"));
}

pub struct Indexer {
    zone_indexer: ZoneIndexer<NodeHttpClient>,
}

fn parse_channel_id(channel_id_str: &str) -> anyhow::Result<ChannelId> {
    let decoded = hex::decode(channel_id_str).map_err(|_| {
        anyhow!(format!("INDEXER_CHANNEL_ID must be a valid hex string, got: '{channel_id_str}'"))
    })?;
    let channel_bytes: [u8; 32] = decoded.try_into().map_err(|v: Vec<u8>| {
        anyhow!(format!(
            "INDEXER_CHANNEL_ID must be exactly 64 hex characters (32 bytes), got {} characters ({} bytes)",
            v.len() * 2,
            v.len()
        ))
    })?;
    Ok(ChannelId::from(channel_bytes))
}

impl Indexer {
    pub fn new(
        // db_path: &str,
        node_endpoint: &str,
        channel_path: &str,
        node_auth_username: Option<String>,
        node_auth_password: Option<String>,
    ) -> anyhow::Result<Self> {
        let node_url = Url::parse(node_endpoint)?;
            // .map_err(|e| anye.to_string()))?;

        let basic_auth = node_auth_username
            .map(|username| BasicAuthCredentials::new(username, node_auth_password));

        let channel_id_str = fs::read_to_string(channel_path)
            .context(format!("Failed to read channel path '{channel_path}'"))
            ?;
        let channel_id = parse_channel_id(channel_id_str.trim())?;

        info!("Channel ID: {}", hex::encode(channel_id.as_ref()));

        let node = NodeHttpClient::new(CommonHttpClient::new(basic_auth), node_url);
        let zone_indexer = ZoneIndexer::new(channel_id, node);

        Ok(Self { zone_indexer })
    }

    pub async fn run(self) {

        loop {
            info!("Connecting to zone block stream...");
            let stream = match self.zone_indexer.follow().await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to connect to block stream: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };
            info!("Connected to zone block stream");

            futures::pin_mut!(stream);
            while let Some(zone_msg) = stream.next().await {
                let logos_blockchain_zone_sdk::ZoneMessage::Block(zone_block) = zone_msg else {
                    continue;
                };

                let data = Vec::from(zone_block.data);
                let price_obs = match PriceObservation::decode(data.as_slice()) {
                    Ok(obs) => obs,
                    Err(err) => {
                        eprint!("Error while decoding zone data: {}", err);
                        continue;
                    }
                };
                println!("[Indexer] price observation: {:?}", price_obs);
            }

            error!("Zone block stream ended, reconnecting...");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}
