// Download worker
//
// Each worker repeatedly issues GET requests to the provided `url` and
// streams response bodies incrementally. The worker updates a shared
// atomic byte counter with the length of each received chunk. The worker
// stops when the provided `deadline` is reached or on any network error.
use futures_util::StreamExt;
use reqwest::Client;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::time::Instant;

/// Continuously download from `url` until `deadline` and add the number
/// of received bytes to `counter`.
///
/// Parameters:
/// - `client`: reqwest `Client` (cloned for each worker)
/// - `url`: download endpoint
/// - `counter`: shared atomic counter to aggregate bytes across workers
/// - `deadline`: instant after which the worker should stop
pub async fn download_worker(
    client: Client,
    url: String,
    counter: Arc<AtomicU64>,
    deadline: Instant,
) {
    while Instant::now() < deadline {
        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            if Instant::now() >= deadline {
                return;
            }

            match chunk {
                Ok(bytes) => {
                    counter.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                }
                Err(_) => return,
            }
        }
    }
}
