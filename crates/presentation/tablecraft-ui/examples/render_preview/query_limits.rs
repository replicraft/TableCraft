//! Source-free illustration of the actual read-query preferences widget.
use egui_kittest::Harness;
use tablecraft_ui::{QueryPoolLimits, QueryRunLimits, preview::Preview};

pub(super) fn prepare(harness: &mut Harness<'_, Preview>) {
    let state = harness.state_mut().state_mut();
    state.set_query_run_limits(QueryRunLimits {
        snapshot_rows: 16_384,
        batch_rows: 256,
        native: QueryPoolLimits {
            bytes: 536_870_912,
            rows: 100_000,
            steps: 5_000_000,
        },
        query: QueryPoolLimits {
            bytes: 16_777_216,
            rows: 10_000,
            steps: 1_000_000,
        },
        comparison: QueryPoolLimits {
            bytes: 16_777_216,
            rows: 0,
            steps: 1_000_000,
        },
        presentation: QueryPoolLimits {
            bytes: 8_388_608,
            rows: 0,
            steps: 100_000,
        },
    });
    assert!(state.open_query_run_limits());
    harness.run_steps(3);
    harness.run();
}
