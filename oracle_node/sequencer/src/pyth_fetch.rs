use anyhow::Context;
use futures::StreamExt;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{sleep, Duration};
// use serde::{Serialize, Deserialize};
use tracing::{info, debug, error};

use common::HermesPriceEvent;

pub async fn fetch_price(
    hermes_price_url: &str,
    hermes_bearer: &str,
    price_id: &str,
    price_update_queue: UnboundedSender<HermesPriceEvent>) -> anyhow::Result<()>
{
    /*
    let eth_usd_id = "ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace";
    let url = format!(
        "https://hermes.pyth.network/v2/updates/price/stream?ids[]={}",
        eth_usd_id
    );
    */

    let url = format!("{}?ids[]={}", hermes_price_url, price_id);

    // Reuse the same HTTP client across reconnections
    let client = Client::new();
    let mut retry_delay = Duration::from_secs(1);

    loop {
        info!("Connecting to Pyth Hermes SSE stream...");

        let mut event_source = EventSource::new(client
            .get(&url)
            .header("Authorization", format!("Bearer {}", hermes_bearer))
        ).context("While creating Pyth Hermes SSE stream conn")?;

        while let Some(event) = event_source.next().await {
            match event {
                Ok(Event::Open) => {
                    info!("Pyth Hermes connection opened successfully!");
                    // Reset the backoff delay on a successful connection
                    retry_delay = Duration::from_secs(1);
                }
                Ok(Event::Message(message)) => {
                    // println!("New Price Update: {}", message.data);

                    match serde_json::from_str::<HermesPriceEvent>(&message.data) {
                        Ok(update_event) => {
                            if let Err(e) = price_update_queue.send(update_event) {
                                error!("Error while sending price update event: {}", e);
                            }
                        },
                        Err(err) => {
                            error!("Failed to parse Pyth JSON: {}\nRaw data: {}", err, message.data);
                        }
                    }
                }
                Err(err) => {
                    error!("Fatal stream error: {}", err);
                    // Close the dead stream so we can rebuild it
                    event_source.close();
                    break; // Break out of the inner while-loop to trigger a reconnect
                }
            }
        }

        // If we reach here, the stream died. Apply a backoff delay before reconnecting.
        info!("Reconnecting in {} seconds...", retry_delay.as_secs());
        sleep(retry_delay).await;

        // Exponentially increase the delay, capped at 30 seconds
        retry_delay = std::cmp::min(retry_delay * 2, Duration::from_secs(30));
    }

    // Ok(())
}

