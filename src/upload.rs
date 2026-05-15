// Upload worker
//
// Each worker repeatedly issues POST requests with a fixed payload to the
// provided `url`. Successful request body sizes are added to the shared
// counter. The worker stops when the provided `deadline` is reached or on
// network error.
use reqwest::Client;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::time::Instant;

const UPLOAD_CHUNK_SIZE: usize = 256 * 1024;

/// Continuously upload to `url` until `deadline` and add uploaded bytes to
/// `counter`.
pub async fn upload_worker(
    client: Client,
    url: String,
    counter: Arc<AtomicU64>,
    deadline: Instant,
) {
    let payload = vec![0x5Au8; UPLOAD_CHUNK_SIZE];

    while Instant::now() < deadline {
        if client
            .post(&url)
            .body(payload.clone())
            .send()
            .await
            .is_err()
        {
            return;
        }

        counter.fetch_add(UPLOAD_CHUNK_SIZE as u64, Ordering::Relaxed);
    }
}
