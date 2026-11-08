//! A real finished review is not a write grant after presentation preflight fails.
use super::*;
use tablecraft_runtime::Response;

#[test]
fn current_native_review_refusal_releases_to_exact_name_without_active_catalog_binding() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    harness.state_mut().state.number_column_available = true;
    harness.state_mut().inspect_number_column_availability();
    finish_reads(&mut harness);
    let tab = harness.state().state.active_object().unwrap();
    assert!(
        harness
            .state_mut()
            .state
            .begin_number_column_name(&tab, "Score".into())
    );
    let request = harness
        .state()
        .state
        .number_column_review_request()
        .unwrap();
    harness.state_mut().save_number_column(request);
    let deadline = Instant::now() + Duration::from_secs(10);
    // Observe the actual runtime response before normal publication. No product
    // pause flag or alternate save-state implementation is introduced here.
    let (generation, result) = loop {
        if let Some(response) = harness.state_mut().worker.poll() {
            break response;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    let Response::NumberColumnReviewed { review, capability } = result.unwrap() else {
        panic!("native named review expected");
    };
    let database = harness.state_mut().state.database.take().unwrap();
    harness
        .state_mut()
        .start_reviewed_number_column(generation, review, capability);
    assert!(harness.state().editing.number_column.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().state.busy);
    assert_eq!(
        harness.state().state.number_column_name().unwrap().1,
        "Score"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    harness.state_mut().state.database = Some(database);
    assert!(harness.state().state.can_edit_number_column_name(&tab));
}
