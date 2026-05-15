// Application runner and shared types
//
// This module exposes a small, testable runner for the speed test. The
// runner publishes stage updates via an optional `watch::Sender` so UI
// front-ends can render progress while the test runs.
use anyhow::Result;
use reqwest::Client;
use std::sync::{Arc, atomic::AtomicU64};
use tokio::{
    sync::watch,
    time::{Duration, Instant},
};

use crate::{api, download, metrics, upload};

/// Simple stage enum used to describe high-level progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppStage {
    FetchingToken,
    FetchingServers,
    RunningDownloadTest,
    RunningUploadTest,
    Complete,
}

pub fn stage_label(stage: AppStage) -> &'static str {
    match stage {
        AppStage::FetchingToken => "Fetching Fast.com token",
        AppStage::FetchingServers => "Fetching test servers",
        AppStage::RunningDownloadTest => "Running download test",
        AppStage::RunningUploadTest => "Running upload test",
        AppStage::Complete => "Complete",
    }
}

/// Configuration for a single speed test run.
#[derive(Clone, Copy, Debug)]
pub struct SpeedTestConfig {
    /// Number of parallel download streams
    pub connections: usize,
    /// Test duration (seconds)
    pub duration: u64,
}

/// Result summary returned by the runner.
#[derive(Clone, Copy, Debug)]
pub struct SpeedTestResult {
    pub download_bytes: u64,
    pub upload_bytes: u64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub phase_duration: u64,
}

/// Build a configured reqwest `Client` used by the runner.
pub fn build_client() -> Result<Client> {
    Ok(Client::builder().user_agent("Mozilla/5.0").build()?)
}

/// Run a single speed test.
///
/// Parameters:
/// - `client`: shared HTTP client
/// - `config`: number of connections and duration
/// - `counter`: shared atomic byte counter updated by download workers
/// - `progress`: optional watch sender used to publish `AppStage` updates
pub async fn run_speed_test(
    client: &Client,
    config: SpeedTestConfig,
    counter: Arc<AtomicU64>,
    progress: Option<watch::Sender<AppStage>>,
) -> Result<SpeedTestResult> {
    if let Some(tx) = &progress {
        let _ = tx.send(AppStage::FetchingToken);
    }

    let token = api::fetch_token(client).await?;

    if let Some(tx) = &progress {
        let _ = tx.send(AppStage::FetchingServers);
    }

    let servers = api::fetch_servers(client, &token, config.connections).await?;

    if let Some(tx) = &progress {
        let _ = tx.send(AppStage::RunningDownloadTest);
    }

    let download_deadline = Instant::now() + Duration::from_secs(config.duration);
    let mut download_handles = Vec::new();

    for server in &servers {
        let client = client.clone();
        let counter = counter.clone();
        let url = server.url.clone();

        download_handles.push(tokio::spawn(async move {
            download::download_worker(client, url, counter, download_deadline).await;
        }));
    }

    for handle in download_handles {
        let _ = handle.await;
    }

    let download_bytes = counter.load(std::sync::atomic::Ordering::Relaxed);
    let download_mbps = metrics::calculate_mbps(download_bytes, config.duration);

    counter.store(0, std::sync::atomic::Ordering::Relaxed);

    if let Some(tx) = &progress {
        let _ = tx.send(AppStage::RunningUploadTest);
    }

    let upload_deadline = Instant::now() + Duration::from_secs(config.duration);
    let mut upload_handles = Vec::new();

    for server in &servers {
        let client = client.clone();
        let counter = counter.clone();
        let url = server.url.clone();

        upload_handles.push(tokio::spawn(async move {
            upload::upload_worker(client, url, counter, upload_deadline).await;
        }));
    }

    for handle in upload_handles {
        let _ = handle.await;
    }

    let upload_bytes = counter.load(std::sync::atomic::Ordering::Relaxed);
    let upload_mbps = metrics::calculate_mbps(upload_bytes, config.duration);

    if let Some(tx) = &progress {
        let _ = tx.send(AppStage::Complete);
    }

    Ok(SpeedTestResult {
        download_bytes,
        upload_bytes,
        download_mbps,
        upload_mbps,
        phase_duration: config.duration,
    })
}
