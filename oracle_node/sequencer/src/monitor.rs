// use tokio::select;
use tokio::sync::mpsc::UnboundedReceiver;
use dashmap::DashMap;
use tracing::info;
// external deps
use common::{HermesPriceEvent, ParsedUpdate};

pub struct PriceMonitor {
    map: DashMap<String, Vec<ParsedUpdate>>,
}

impl PriceMonitor {

    pub fn new(map: DashMap<String, Vec<ParsedUpdate>>) -> Self {
        Self { map }
    }

    pub async fn run(&self,
                     price_update_queue: &mut UnboundedReceiver<HermesPriceEvent>,
                     /* price_request_queue: &mut UnboundedReceiver<> */) -> anyhow::Result<()> {

        info!("Starting price monitor...");

        loop {
            let price_update = price_update_queue.recv().await;
            let Some(price_update) = price_update else { eprintln!("Channel closed"); break; };

            // println!("Got price update: {:?}", price_update);
            if let Some(mut parsed) = price_update.parsed {
                let parsed_0 = std::mem::take(&mut parsed[0]);
                // To avoid a clone here (on parsed_0, when using or_insert) we expect the entry to be already present
                self.map
                    .entry(parsed_0.id.clone())
                    .and_modify(|e| e.push(parsed_0))
                    ;
            }
        }

        Ok(())
    }
}