//! Host admission bookkeeping; these cases do not fabricate storage completion.
use super::*;
use crate::workspace::test_support::opened;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_builders::table::TableDraft;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

#[test]
fn unrelated_host_save_refusal_returns_the_exact_issued_request_to_naming() {
    let (_directory, path, original, mut harness) = opened(canonical_blank_creation::source());
    assert!(harness.state_mut().state.open_initial_table_draft());
    assert!(harness.state_mut().state.request_table_draft_save());
    harness.run_steps(2);

    // Isolate the host admission boundary. This is another operation's UI
    // bookkeeping, not a forged runtime save receipt or a claimed file lease.
    harness.state_mut().editing.save = Some(ActiveSave {
        owner: SaveOwner::Cell,
        id: 71,
        path: path.clone(),
        table: "Other operation".into(),
        offset: 0,
        report: None,
    });
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(1);

    assert!(harness.state().first_table.is_none());
    assert!(harness.state().state.table_draft_save_open());
    assert!(!harness.state().state.table_draft_save_pending());
    assert!(!harness.state().state.busy);
    assert_eq!(harness.state().editing.save.as_ref().unwrap().id, 71);
    assert!(harness.state().editing.worker.poll().is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());

    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Table name")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("The table could not be saved.")
            .is_some()
    );
}

#[test]
fn another_request_cannot_reset_the_existing_first_table_review_owner() {
    let (_directory, path, original, mut harness) = opened(canonical_blank_creation::source());
    assert!(harness.state_mut().state.open_initial_table_draft());
    assert!(harness.state_mut().state.request_table_draft_save());
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(1);

    // Review may finish on its worker, but no later host frame has consumed it.
    // A second direct request must preserve that original pending capability.
    let owner = harness.state().first_table.as_ref().unwrap();
    let request_id = owner.request;
    let tab = owner.tab.clone();
    let revision = owner.revision;
    let generation = harness.state().generation;
    let snapshot = TableDraft::initial().snapshot("MustNotReplace").unwrap();
    harness.state_mut().save_first_table(TableSaveRequest {
        id: u64::MAX,
        tab: tab.clone(),
        snapshot,
    });

    let owner = harness.state().first_table.as_ref().unwrap();
    assert_eq!(owner.request, request_id);
    assert_eq!(owner.tab, tab);
    assert_eq!(owner.revision, revision);
    assert_eq!(harness.state().generation, generation);
    assert!(harness.state().state.table_draft_save_pending());
    assert!(matches!(
        harness.state().pending,
        PendingOperation::FirstTableReview { request_id: current } if current == request_id
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());

    harness.state_mut().cancel_first_table_save(request_id);
    assert!(harness.state().first_table.is_none());
    assert!(!harness.state().state.table_draft_save_pending());
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
