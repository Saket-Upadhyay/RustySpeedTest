use rusty_speed_test::metrics;

#[test]
fn calculate_mbps_handles_zero_seconds() {
    let mbps = metrics::calculate_mbps(1_000_000, 0);
    assert_eq!(mbps, 0.0);
}

#[test]
fn bytes_to_mb_converts_units() {
    let mb = metrics::bytes_to_mb(1024 * 1024);
    assert!((mb - 1.0).abs() < f64::EPSILON);
}
