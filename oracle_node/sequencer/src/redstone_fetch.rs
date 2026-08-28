use serde::{Serialize, Deserialize};
use reqwest::Client;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedstonePriceEvent {
    /// The asset symbol (e.g., "BTC")
    pub symbol: String,

    /// The oracle provider (e.g., "redstone")
    pub provider: String,

    /// The resolved price (RedStone sends this as an f64)
    pub value: f64,

    /// Timestamp of the price resolution (Unix epoch in milliseconds)
    pub timestamp: u64,
}

pub async fn fetch_price(symbol: &str, provider: &str, price_update_queue: UnboundedSender<PartialPriceObservation>) {
    // RedStone public Gateway API
    let url = format!("https://api.redstone.finance/prices?symbol={}&provider={}", symbol, provider);

    // Always reuse client
    let client = Client::new();
    let mut retry_delay = Duration::from_secs(1);

    // Polling interval
    let poll_interval = Duration::from_secs(3);

    loop {
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    retry_delay = Duration::from_secs(1);

                    match response.text().await {
                        Ok(text) => {
                            match serde_json::from_str::<RedstonePriceEvent>(&text) {
                                Ok(update_event) => {
                                    if let Err(e) = price_update_queue.send(update_event) {
                                        eprintln!("Error while sending price update event: {}", e);
                                    }
                                },
                                Err(err) => {
                                    eprintln!("Failed to parse RedStone JSON: {}\nRaw data: {}", err, text);
                                }
                            }
                        }
                        Err(e) => eprintln!("Failed to read response text: {}", e),
                    }
                    
                    sleep(poll_interval).await;
                    continue;
                } else {
                    eprintln!("RedStone API returned status: {}", response.status());
                }
            }
            Err(err) => {
                eprintln!("Fatal request error: {}", err);
            }
        }

        // Network or Http error, retry with backoff
        println!("Retrying in {} seconds...", retry_delay.as_secs());
        sleep(retry_delay).await;

        // Exponential backoff capped at 30 seconds
        retry_delay = std::cmp::min(retry_delay * 2, Duration::from_secs(30));
    }
}