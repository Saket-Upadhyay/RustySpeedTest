// Entry point and CLI glue
//
// Provides a small CLI wrapper and selects the TUI by default when a
// terminal is available. Use `--no-tui` to force plain CLI output.
mod api;
mod app;
mod download;
mod metrics;
mod tui;
mod upload;

use anyhow::Result;
use clap::Parser;
use std::{
    io::IsTerminal,
    sync::{Arc, atomic::AtomicU64},
};
use tokio::sync::watch;

use crate::app::{AppStage, SpeedTestConfig, build_client, run_speed_test, stage_label};

/// Command-line arguments for the test runner.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Number of parallel download streams
    #[arg(short, long, default_value_t = 4)]
    connections: usize,

    /// Test duration in seconds
    #[arg(short, long, default_value_t = 8)]
    duration: u64,

    /// Disable TUI and fall back to CLI output
    #[arg(long, default_value_t = false)]
    no_tui: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = SpeedTestConfig {
        connections: args.connections,
        duration: args.duration,
    };

    let use_tui = !args.no_tui && std::io::stdout().is_terminal();

    if use_tui {
        return tui::run_tui(config).await;
    }

    run_cli(config).await
}

/// Run the test in non-interactive CLI mode. This function reuses the
/// shared runner and prints stage updates to stdout.
async fn run_cli(config: SpeedTestConfig) -> Result<()> {
    let client = build_client()?;
    let counter = Arc::new(AtomicU64::new(0));
    let (tx, mut rx) = watch::channel(AppStage::FetchingToken);

    let printer = tokio::spawn(async move {
        let mut last = None;

        loop {
            let stage = *rx.borrow();
            if last != Some(stage) {
                println!("{}...", stage_label(stage));
                last = Some(stage);
            }

            if rx.changed().await.is_err() {
                break;
            }
        }
    });

    let result = run_speed_test(&client, config, counter, Some(tx)).await?;

    let _ = printer.await;

    println!();
    println!("Download speed: {:.2} MBps", result.download_mbps);
    println!("Upload speed: {:.2} MBps", result.upload_mbps);

    Ok(())
}
