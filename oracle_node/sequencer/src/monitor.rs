use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use dashmap::DashMap;
use tracing::{info, error};
// external deps
use common::PartialPriceObservation;

pub struct PriceMonitor {
    map: Arc<DashMap<String, Vec<PartialPriceObservation>>>,
}

impl PriceMonitor {

    pub fn new(map: Arc<DashMap<String, Vec<PartialPriceObservation>>>) -> Self {
        Self { map }
    }

    pub async fn run(&self,
                     price_update_queue: &mut UnboundedReceiver<PartialPriceObservation>,
    ) -> anyhow::Result<()> {

        info!("Starting price monitor...");

        loop {
            let price_update = price_update_queue.recv().await;
            let Some(price_update) = price_update else { error!("Channel closed"); break; };

            let feed_id_key = price_update.feed_id.clone();
            self
                .map
                .entry(feed_id_key)
                .and_modify(|v| v.push(price_update.clone()))
                .or_insert(vec![price_update]);
        }

        Ok(())
    }
}