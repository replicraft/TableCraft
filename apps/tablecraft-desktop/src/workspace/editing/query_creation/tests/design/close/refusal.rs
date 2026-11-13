//! Real existing-fragment refusal cannot resume a deferred Open or discard Design.
use super::super::super::{opened, saved};
use super::super::design;
use super::{choose, close, edit};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::{Command, journal_path};
use tablecraft_ui::SavedQuerySqlError;

#[test]
fn unsupported_saved_alias_close_save_cancels_deferred_open_and_retains_the_actual_dirty_design() {
    let (directory, path, case, clock, mut harness) = opened();
    saved(&mut harness, "Saved");
    let tab = harness.state().state.active_object().unwrap();
    // NO_CLONE: Preserve an independent exact baseline while the same live saved
    // editor builds and retains a different proposed SQL string after refusal.
    let original = harness
        .state()
        .state
        .saved_query_sql()
        .unwrap()
        .0
        .to_string();
    design(&mut harness);
    // This is valid QBE. The existing native expression-fragment operation
    // deliberately preserves aliases, so real adapter diff rejects this change.
    edit(&mut harness, "OtherHeading: 43");
    assert!(
        !harness.state().state.saved_query_sql_dirty(),
        "typed Design has not been built into proposed SQL yet"
    );
    let target = harness.state().state.selected_saved_target().unwrap();
    assert!(!harness.state().state.can_run_saved_query(target, &[]));
    let other = directory.path().join("other.accdb");
    std::fs::write(&other, &case.before).unwrap();
    // NO_CLONE: The actual deferred Open owns its request path while the test
    // independently verifies the other complete source was neither opened nor edited.
    harness.state_mut().open_request(Command::Open {
        path: other.clone(),
        password: String::new(),
    });
    harness.run_steps(2);
    assert!(harness.state().state.query_design_close_open());
    assert!(harness.state().deferred_draft_action.is_some());
    choose(&mut harness, "saved-query-close-save");
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy
        || harness.state().editing.has_save()
        || harness.state().deferred_draft_action.is_some()
    {
        assert!(
            Instant::now() < deadline,
            "real existing-query refusal must retire its exact pending close and Open"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
    let Some(SavedQuerySqlError::Host(message)) = harness.state().state.saved_query_sql_error()
    else {
        panic!("actual TCQE alias-change review must return its host profile refusal");
    };
    assert!(message.contains("existing expression-fragment profile"));
    assert!(!harness.state().state.query_creation_name_open());
    assert!(!harness.state().state.query_design_close_open());
    assert!(harness.state().deferred_draft_action.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert!(harness.state().state.has_modified_query_design());
    let (stored, proposed, _, _) = harness.state().state.saved_query_sql().unwrap();
    assert_eq!(stored, original);
    assert!(proposed.contains("OtherHeading") && proposed.contains("43"));
    assert!(harness.state().state.saved_query_sql_dirty());
    assert_eq!(
        harness.state().state.query_design_draft().unwrap().columns[0]
            .alias
            .as_deref(),
        Some("OtherHeading")
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("OtherHeading: 43")
    );
    assert_eq!(
        clock.calls(),
        0,
        "existing-query refusal must not enter new-object review"
    );
    assert_eq!(std::fs::read(&path).unwrap(), case.before);
    assert_eq!(std::fs::read(&other).unwrap(), case.before);
    assert!(!journal_path(&path).exists());
    assert!(!journal_path(&other).exists());

    // A refusal must leave a usable original editor, not a hidden close intent
    // that blocks the next actual close decision or resumes a cancelled Open.
    close(&mut harness);
    choose(&mut harness, "saved-query-close-cancel");
    harness.run_steps(2);
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert!(harness.state().state.has_modified_query_design());
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(path).unwrap(), case.before);
}
