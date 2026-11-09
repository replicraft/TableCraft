use super::*;

#[test]
fn explicit_host_limits_round_trip_through_the_real_checked_runtime_constructor() {
    let default = QueryRunOptions::default();
    assert_eq!(runtime_options(ui_limits(default)).unwrap(), default);
    let mut limits = ui_limits(default);
    limits.snapshot_rows = 100_000;
    limits.native.rows = 200_000;
    limits.query.rows = 400_000;
    limits.query.steps = 10_000_000;
    let options = runtime_options(limits).unwrap();
    assert_eq!(ui_limits(options), limits);

    limits.comparison.rows = 1;
    assert_eq!(
        runtime_options(limits),
        Err(QueryRunOptionsError::NonRowPool)
    );
    limits.comparison.rows = 0;
    limits.snapshot_rows = 0;
    assert_eq!(
        runtime_options(limits),
        Err(QueryRunOptionsError::SnapshotBounds)
    );
}
