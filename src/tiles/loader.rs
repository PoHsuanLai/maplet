use std::sync::mpsc::Sender;
use std::thread;
use crate::core::geo::TileCoord;
use super::source::TileSource;
use crate::Result;
use once_cell::sync::Lazy;
use reqwest::blocking::Client;

/// Shared blocking HTTP client with a custom User-Agent so that public tile
/// servers (e.g. OpenStreetMap) don't reject the request. Building the client
/// once avoids the cost of TLS and connection pool setup for every tile.
pub(crate) static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("map-rs/0.1 (+https://github.com/example/map-rs)")
        .build()
        .expect("failed to build reqwest blocking client")
});

/// Simple tile loader that fetches tiles in background threads and sends the
/// resulting bytes back over an `mpsc` channel.
pub struct TileLoader {
    tx: Sender<(TileCoord, Vec<u8>)>,
}

impl TileLoader {
    /// Create a new tile loader given a sender to report completed downloads.
    pub fn new(tx: Sender<(TileCoord, Vec<u8>)>) -> Self {
        Self { tx }
    }

    /// Start downloading the specified tile. The download occurs on a detached
    /// thread so that it does not block the caller. When the request finishes
    /// (successfully or not), the provided sender will receive the tile bytes.
    pub fn start_download(&self, source: &dyn TileSource, coord: TileCoord) {
        let url = source.url(coord);
        let tx_clone = self.tx.clone();

        thread::spawn(move || {
            const MAX_ATTEMPTS: usize = 2;
            for attempt in 1..=MAX_ATTEMPTS {
                log::debug!("fetch tile {:?} attempt {}", coord, attempt);
                let result: Result<Vec<u8>> = (|| {
                    let resp = HTTP_CLIENT.get(&url).send()?;
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status()).into());
                    }
                    let bytes = resp.bytes()?;
                    Ok(bytes.to_vec())
                })();

                match result {
                    Ok(data) => {
                        log::info!("downloaded tile {:?} ({} bytes)", coord, data.len());
                        let _ = tx_clone.send((coord, data));
                        return; // success
                    }
                    Err(e) => {
                        log::warn!("tile {:?} download failed on attempt {}: {}", coord, attempt, e);
                        if attempt == MAX_ATTEMPTS {
                            log::error!("giving up on tile {:?}", coord);
                        } else {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                }
            }
        });
    }
} 