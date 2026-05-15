// Terminal UI (TUI) implementation
//
// Small, focused TUI that autoruns the speed test, renders live
// throughput and stage information, and provides light controls: quit
// and rerun. The implementation separates terminal setup/teardown from
// final summary printing so results remain visible after the UI exits.
use anyhow::{Result, anyhow};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::{
    io::{self, Stdout},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::watch,
    time::{Duration as TokioDuration, Instant},
};

use crate::{
    app::{AppStage, SpeedTestConfig, SpeedTestResult, build_client, run_speed_test, stage_label},
    metrics,
};

/// TerminalGuard ensures raw mode is disabled on panic/exit.
///
/// Note: leaving the alternate screen is handled explicitly by
/// `run_tui` so a textual summary can be printed to the main screen
/// after the UI finishes.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

/// Start the interactive terminal UI and run the speed test.
///
/// The UI will autorun a single test on launch. Controls:
/// - `q` / `Esc`: quit immediately
/// - `Enter`: quit after completion
/// - `r`: rerun after completion
pub async fn run_tui(config: SpeedTestConfig) -> Result<()> {
    let (mut terminal, _guard) = init_terminal()?;

    let client = build_client()?;
    let counter = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let (mut tx, mut rx) = watch::channel(AppStage::FetchingToken);

    let mut test_handle = {
        let counter = counter.clone();
        let client_clone = client.clone();
        tokio::spawn(async move { run_speed_test(&client_clone, config, counter, Some(tx)).await })
    };

    let mut result: Option<Result<SpeedTestResult>> = None;
    let mut interval = tokio::time::interval(TokioDuration::from_millis(200));
    let mut prev_bytes: u64 = 0;
    let mut last_sample = Instant::now();
    let mut completion_deadline: Option<Instant> = None;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // sample
            },
            res = &mut test_handle, if result.is_none() => {
                result = Some(res.map_err(|err| anyhow!("Task failed: {err}"))?);
                completion_deadline = Some(Instant::now() + TokioDuration::from_secs(5));
            },
            key = read_key_event() => {
                if let Some(key) = key? {
                    // Rerun when finished
                    if key.code == KeyCode::Char('r') && result.is_some() {
                        // reset counters and start a fresh run
                        counter.store(0, Ordering::Relaxed);
                        prev_bytes = 0;
                        last_sample = Instant::now();
                        completion_deadline = None;

                        let (new_tx, new_rx) = watch::channel(AppStage::FetchingToken);
                        tx = new_tx;
                        rx = new_rx;

                        test_handle = {
                            let counter = counter.clone();
                            let client_clone = client.clone();
                            tokio::spawn(async move {
                                run_speed_test(&client_clone, config, counter, Some(tx)).await
                            })
                        };

                        result = None;
                        continue;
                    }

                    if should_quit(key, result.is_some()) {
                        break;
                    }
                }
            }
        }

        // update sampling
        let bytes = counter.load(Ordering::Relaxed);
        let now = Instant::now();
        let delta_bytes = bytes.saturating_sub(prev_bytes);
        let delta_secs = now.duration_since(last_sample).as_secs_f64().max(1e-6);
        let _sample_mbps = (delta_bytes as f64) / delta_secs / 1_000_000.0;
        prev_bytes = bytes;
        last_sample = now;
        draw_ui(&mut terminal, &rx, &counter, start, config, result.as_ref())?;

        // handle auto-exit after completion
        if let Some(deadline) = completion_deadline
            && Instant::now() >= deadline
        {
            break;
        }
    }

    // Before returning, restore the main screen and print the final results
    // so they remain visible after the TUI exits.
    let bytes = counter.load(Ordering::Relaxed);
    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs().max(1);
    let final_mbps = metrics::calculate_mbps(bytes, elapsed_secs);

    // leave alternate screen and disable raw mode for normal terminal output
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);

    println!();
    if let Some(res) = result {
        match res {
            Ok(done) => {
                println!("RustySpeedTest completed");
                println!("=========================");
                println!("Download: {:.2} MBps", done.download_mbps);
                println!("Upload: {:.2} MBps", done.upload_mbps);
                println!("=========================");
                println!(
                    "Downloaded: {:.2} MB",
                    metrics::bytes_to_mb(done.download_bytes)
                );
                println!(
                    "Uploaded: {:.2} MB",
                    metrics::bytes_to_mb(done.upload_bytes)
                );
                println!("Per-phase duration: {}s", done.phase_duration);
                println!("=========================");
            }
            Err(err) => {
                println!("Speed test failed: {}", err);
            }
        }
    } else {
        println!("Partial result: {:.2} MBps", final_mbps);
        println!("Downloaded: {:.2} MB", metrics::bytes_to_mb(bytes));
    }

    Ok(())
}

fn init_terminal() -> Result<(Terminal<CrosstermBackend<Stdout>>, TerminalGuard)> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    Ok((terminal, TerminalGuard))
}

fn should_quit(key: KeyEvent, finished: bool) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Enter => finished,
        _ => matches!(key.modifiers, KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'),
    }
}

async fn read_key_event() -> Result<Option<KeyEvent>> {
    tokio::task::spawn_blocking(|| -> Result<Option<KeyEvent>> {
        if event::poll(Duration::from_millis(0))?
            && let Event::Key(key) = event::read()?
        {
            return Ok(Some(key));
        }
        Ok(None)
    })
    .await
    .map_err(|err| anyhow!("Event task failed: {err}"))?
}

fn draw_ui(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    stage_rx: &watch::Receiver<AppStage>,
    counter: &Arc<AtomicU64>,
    start: Instant,
    config: SpeedTestConfig,
    result: Option<&Result<SpeedTestResult>>,
) -> Result<()> {
    let stage = *stage_rx.borrow();
    let bytes = counter.load(Ordering::Relaxed);
    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs().max(1);
    let live_mbps = metrics::calculate_mbps(bytes, elapsed_secs);
    let downloaded_mb = metrics::bytes_to_mb(bytes);

    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(5),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(frame.area());

        let header = Paragraph::new(Line::from(Span::styled(
            "RustySpeedTest x DrDope",
            Style::default().fg(Color::Cyan),
        )))
        .block(Block::default().borders(Borders::ALL).title(""));
        frame.render_widget(header, chunks[0]);

        let status_lines = vec![
            Line::from(Span::raw(format!("Stage: {}", stage_label(stage)))),
            Line::from(Span::raw(format!(
                "Connections: {} | Duration: {}s",
                config.connections, config.duration
            ))),
        ];
        let status =
            Paragraph::new(status_lines).block(Block::default().borders(Borders::ALL).title("Run"));
        frame.render_widget(status, chunks[1]);

        let metrics_lines = if let Some(res) = result {
            match res {
                Ok(done) => vec![
                    Line::from(Span::styled(
                        format!("Download: {:.2} MBps", done.download_mbps),
                        Style::default().fg(Color::Green),
                    )),
                    Line::from(Span::raw(format!(
                        "Downloaded: {:.2} MB",
                        metrics::bytes_to_mb(done.download_bytes)
                    ))),
                    Line::from(Span::styled(
                        format!("Upload: {:.2} MBps", done.upload_mbps),
                        Style::default().fg(Color::Cyan),
                    )),
                    Line::from(Span::raw(format!(
                        "Uploaded: {:.2} MB",
                        metrics::bytes_to_mb(done.upload_bytes)
                    ))),
                    Line::from(Span::raw(format!(
                        "Per-phase duration: {}s",
                        done.phase_duration
                    ))),
                ],
                Err(err) => vec![
                    Line::from(Span::styled(
                        "Speed test failed",
                        Style::default().fg(Color::Red),
                    )),
                    Line::from(Span::raw(err.to_string())),
                ],
            }
        } else {
            let v = vec![
                Line::from(Span::raw(format!("Elapsed: {}s", elapsed_secs))),
                Line::from(Span::raw(format!("Transferred: {:.2} MB", downloaded_mb))),
                Line::from(Span::raw(format!("Estimated: {:.2} MBps", live_mbps))),
            ];

            v
        };

        let metrics_block = Paragraph::new(metrics_lines)
            .block(Block::default().borders(Borders::ALL).title("Metrics"));
        frame.render_widget(metrics_block, chunks[2]);

        let footer_text = if result.is_some() {
            "Press Enter or q to quit | r to rerun"
        } else {
            "Press q to quit"
        };
        let footer = Paragraph::new(Line::from(Span::raw(footer_text)))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[3]);
    })?;

    Ok(())
}
