//! A renamed sort key fails explicitly; its opaque preference is never rewritten.
use super::accept_review;
use crate::workspace::test_support;
use crate::workspace::test_support::finish_reads;
use egui_kittest::kittest::Queryable;
use std::time::{Duration, Instant};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::SourceCase, saved_field_rename,
};
use tablecraft_ui::SortSpec;

#[test]
fn renamed_field_sort_is_retained_after_explicit_fresh_schema_restoration_failure() {
    let [before, after] = saved_field_rename::pair(SourceCase::Number);
    let (_directory, path, _, mut harness) = test_support::opened(before);
    let original_sort = SortSpec {
        column: "Score".into(),
        descending: true,
    };
    let target = harness.state().state.current_sort_target().unwrap();
    harness
        .state_mut()
        .sort_view(target, Some(original_sort.clone()), false);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.table_sort(),
        Some(original_sort.clone())
    );
    assert!(harness.state().filter.is_some());

    accept_review(&mut harness, "Total");
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.has_save() {
        assert!(
            Instant::now() < deadline,
            "rename/sort restoration did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().state.field_rename_open());
    // Fresh-catalog filter failures have their own retained report, not the
    // global workspace error. Require its actual visible native diagnosis.
    harness.run_steps(2);
    harness.get_by_label("native column has no binding");
    assert_eq!(harness.state().state.table_sort(), Some(original_sort));
    assert!(
        harness.state().filter.is_none(),
        "old source-bound handle must be invalidated"
    );
    assert!(
        harness.state().state.data.is_none(),
        "failed restoration must not publish an unfiltered fallback"
    );
    let retained = harness
        .state_mut()
        .capture_filter_restore_definitions()
        .unwrap();
    assert_eq!(
        retained.len(),
        1,
        "opaque failed preference remains available for explicit retry"
    );
    assert_eq!(retained[0].table(), "Table1");
}
