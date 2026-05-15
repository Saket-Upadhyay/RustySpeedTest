// Metrics helpers
//
// Small collection of helpers used to convert aggregate bytes and time
// into human-friendly throughput measures.

/// Calculate megabytes-per-second from `bytes` transferred over `seconds`.
pub fn calculate_mbps(bytes: u64, seconds: u64) -> f64 {
    if seconds == 0 {
        return 0.0;
    }

    (bytes as f64) / (seconds as f64) / 1_000_000.0
}

/// Calculate kilobits-per-second from `bytes` downloaded over `seconds`.
///
/// This helper is kept for convenience; it is intentionally allowed to
/// remain unused in some build profiles.
#[allow(dead_code)]
pub fn calculate_kbps(bytes: u64, seconds: u64) -> f64 {
    if seconds == 0 {
        return 0.0;
    }

    (bytes as f64 * 8.0) / (seconds as f64) / 1_000.0
}

/// Convert bytes to megabytes (MiB-like calculation using 1024^2).
pub fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}
