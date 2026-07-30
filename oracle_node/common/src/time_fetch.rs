use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::sleep;
use lb_http_api_common::paths;

/// Logos rest API
/// https://deepwiki.com/logos-blockchain/logos-blockchain/7.1-http-api-reference
///
/// Time info
/// https://github.com/logos-blockchain/logos-blockchain/blob/e425bac1f4d403ef3369db1100887c65a5f75b9a/nodes/node/binary/src/api/handlers.rs#L488

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeInfo {
    pub slot_duration_ms: u64,
    pub genesis_time_unix_ms: u64,
    pub current_slot: u64,
    pub current_epoch: u32,
}

pub fn time_poller(
    api_base_url: String,
    poll_interval: Duration
) -> watch::Receiver<Option<TimeInfo>> {

    println!("Starting time fetch");
    println!("api_base_url: {}", api_base_url);
    println!("path: {}", paths::TIME_INFO);

    // Create the channel with an initial empty state
    let (tx, rx) = watch::channel(None);

    tokio::spawn(async move {
        let client = Client::new();
        let url = format!("{}{}", api_base_url.trim_end_matches('/'), paths::TIME_INFO);
        println!("url: {}", url);

        loop {
            match client.get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    match response.json::<TimeInfo>().await {
                        Ok(time_info) => {
                            if tx.send(Some(time_info)).is_err() {
                                // If this fails, it means all receivers have been dropped,
                                // so we can safely exit the background loop.
                                eprintln!("Time poller exiting: all receivers dropped.");
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse TimeInfo JSON: {}", e);
                        }
                    }
                }
                Ok(response) => {
                    eprintln!("Time API returned an error status: {}", response.status());
                }
                Err(e) => {
                    eprintln!("Failed to reach the Time API: {}", e);
                }
            }

            sleep(poll_interval).await;
        }
    });

    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_time_info() {

        let slot_duration_ms = 200;
        let genesis_time_unix_ms = 1000;
        let current_slot = 42;
        let current_epoch = 12;
        let ti = TimeInfo {
            slot_duration_ms,
            genesis_time_unix_ms,
            current_slot,
            current_epoch,
        };

        // mock http server (handling time info)
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(paths::TIME_INFO))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(serde_json::to_string(&ti).expect("Cannot ser ti"))
                    .insert_header("content-type", "application/json"),
            )
            .mount(&mock_server)
            .await;
        let mock_url = format!("{}{}", mock_server.uri(), paths::TIME_INFO);
        println!("mock_url: {}", mock_url);

        // Start time poller
        let time_rx = time_poller(mock_server.uri(), Duration::from_secs(1));

        // Wait until the poller fetches the first valid state
        let mut initialized_rx = time_rx.clone();
        let _ = initialized_rx.wait_for(|state| state.is_some()).await;

        let oracle_worker_rx = time_rx.clone();
        if let Some(time_info) = oracle_worker_rx.borrow().as_ref() {
            assert_eq!(*time_info, ti);
        }

    }
}
