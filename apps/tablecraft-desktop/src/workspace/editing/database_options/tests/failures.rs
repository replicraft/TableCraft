//! Real native ACK and read publication boundaries, with no invented outcomes.
use super::*;
use egui::Key;
use egui_kittest::{Harness, kittest::NodeT};
use std::path::PathBuf;
use tablecraft_runtime::{DatabaseOptionsReview, Response, ReviewedDatabaseOptions};
use tablecraft_ui::{
    DatabaseOptionsSaveCommand, DatabaseOptionsSaveIdentity, EditSaveOutcome, ObjectTab,
};

const FIXTURE_PAGE_BYTES: usize = 4096;
const UNUSED_PROPERTY_GAP_OFFSET: usize = 128;
const SCRATCH_SQL: &str = "  SELECT 17 AS [Scratch]; -- preserve exact spelling\n";

type Opened = (
    tempfile::TempDir,
    PathBuf,
    Vec<u8>,
    Harness<'static, Desktop>,
);

fn workspace() -> Opened {
    let before = canonical_creator_rename::originals().remove(0);
    let expected = canonical_database_options::explicit_off(&before);
    let (directory, path, _, mut harness) = test_support::opened(before);
    let persisted = harness.state().state.active_object().unwrap();
    harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: persisted.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql(SCRATCH_SQL.into())
    );
    harness.state_mut().activate_object(persisted);
    finish_reads(&mut harness);
    (directory, path, expected, harness)
}

fn click(harness: &mut Harness<'_, Desktop>, key: &str) {
    let label = harness.state().state.localizer.text(key);
    harness
        .get_all_by_role_and_label(Role::Button, &label)
        .find(|node| !node.accesskit_node().is_disabled())
        .unwrap_or_else(|| panic!("enabled button missing: {key}"))
        .click_accesskit();
    // Actions are consumed after drawing. Paint the resulting dialog before
    // querying its controls rather than using the previous frame's tree.
    harness.run_steps(2);
}

fn take_review(
    harness: &Harness<'_, Desktop>,
) -> (u64, DatabaseOptionsReview, ReviewedDatabaseOptions) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((id, result)) = harness.state().worker.poll() {
            let Response::DatabaseOptionsReviewed { review, capability } = result.unwrap() else {
                panic!("actual settings review expected");
            };
            return (id, review, capability);
        }
        assert!(
            Instant::now() < deadline,
            "native settings review timed out"
        );
        std::thread::yield_now();
    }
}

fn wait_native_completion(harness: &Harness<'_, Desktop>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(report) = harness.state().editing.worker.poll() {
            assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
            assert_eq!(
                report.id,
                harness
                    .state()
                    .editing
                    .database_options
                    .as_ref()
                    .unwrap()
                    .identity
                    .unwrap()
                    .save_id
            );
            return;
        }
        assert!(Instant::now() < deadline, "native settings save timed out");
        std::thread::yield_now();
    }
}

fn cancel_acknowledged_checked_read(
    harness: &mut Harness<'_, Desktop>,
) -> DatabaseOptionsSaveIdentity {
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert_eq!(
        harness.state().state.database_options_available,
        tablecraft_runtime::SaveWorker::supports_database_options()
    );
    harness.state_mut().inspect_database_options(revision);
    finish_reads(harness);
    assert!(
        harness
            .state_mut()
            .state
            .request_database_options_close_confirmation()
    );
    let request = harness
        .state_mut()
        .state
        .approve_database_options_close()
        .unwrap();
    harness.state_mut().disable_name_auto_correct(request);
    let (id, review, capability) = take_review(harness);
    harness
        .state_mut()
        .start_reviewed_database_options(id, review, capability);
    let identity = harness
        .state()
        .editing
        .database_options
        .as_ref()
        .unwrap()
        .identity
        .unwrap();
    wait_native_completion(harness);
    // The actual terminal mailbox remains repeatable until ACK. Use the real
    // controller cancellation latch before frames can auto-ack the completion.
    harness
        .state_mut()
        .database_options_save_command(identity, DatabaseOptionsSaveCommand::Cancel);
    harness.state_mut().poll_saves();
    assert!(harness.state().editing.save.is_some());
    harness.state_mut().acknowledge_database_options(identity);
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    let read_id = harness.state().generation;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((generation, result)) = harness.state().worker.poll() {
            assert_eq!(generation, read_id);
            assert!(matches!(result, Ok(Response::Opened { .. })));
            break;
        }
        assert!(Instant::now() < deadline, "checked settings read timed out");
        std::thread::yield_now();
    }
    let host = harness.state().editing.database_options.as_ref().unwrap();
    let witness = Arc::clone(host.witness.as_ref().unwrap());
    let terminal = host.terminal;
    let cleanup = host.cleanup;
    // A real completed read is held out of host publication, making the actual
    // Escape path deterministic without a product pause flag or fake response.
    let foreign = DatabaseOptionsSaveIdentity {
        save_id: identity.save_id.checked_add(1).unwrap(),
        ..identity
    };
    harness
        .state_mut()
        .database_options_save_command(foreign, DatabaseOptionsSaveCommand::Cancel);
    assert!(harness.state().worker.publication_is_current(read_id));
    assert!(harness.state().state.busy);
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().worker.publication_is_current(read_id));
    assert!(!harness.state().state.busy);
    assert!(
        !harness
            .state()
            .editing
            .database_options
            .as_ref()
            .unwrap()
            .reopening
    );
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    let host = harness.state().editing.database_options.as_ref().unwrap();
    assert!(Arc::ptr_eq(host.witness.as_ref().unwrap(), &witness));
    assert_eq!(host.terminal, terminal);
    assert_eq!(host.cleanup, cleanup);
    assert_eq!(harness.state().state.error, None);
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        revision
    );
    assert!(matches!(
        harness.state().state.database_options_save_status(),
        Some((actual, Some(EditSaveOutcome::Saved(_)))) if actual == identity
    ));
    identity
}

fn assert_old_graph(harness: &Harness<'_, Desktop>, tabs: &[ObjectTab]) {
    assert_eq!(harness.state().state.object_tabs(), tabs);
    assert_eq!(harness.state().state.sql, SCRATCH_SQL);
    assert!(harness.state().state.native_mutation_blocked());
}

#[test]
fn acknowledged_after_escape_cancels_only_read_then_retry_publishes_exact_confirmed_graph() {
    let (_directory, path, expected, mut harness) = workspace();
    let tabs = harness.state().state.object_tabs();
    let identity = cancel_acknowledged_checked_read(&mut harness);
    assert_old_graph(&harness, &tabs);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    click(&mut harness, "saved-query-save-retry-reopen");
    finish_reads(&mut harness);
    assert!(harness.state().editing.database_options.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(
        harness
            .state()
            .state
            .database_options_save_status()
            .is_none()
    );
    let fresh = harness.state().state.object_tabs();
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].id, tabs[1].id);
    assert_ne!(
        fresh[0].target.document_revision,
        identity.document_revision
    );
    assert_eq!(harness.state().state.sql, SCRATCH_SQL);
    assert_eq!(std::fs::read(path).unwrap(), expected);
    assert!(!harness.state().state.native_source_stale());
}

#[test]
fn acknowledged_after_witness_drift_return_preserves_saved_warning_until_fresh_open() {
    let (_directory, path, expected, mut harness) = workspace();
    let tabs = harness.state().state.object_tabs();
    let identity = cancel_acknowledged_checked_read(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let mut changed = expected;
    let unused = changed.len() - FIXTURE_PAGE_BYTES + UNUSED_PROPERTY_GAP_OFFSET;
    assert_eq!(changed[unused], 0);
    changed[unused] = 1;
    std::fs::write(&path, &changed).unwrap();
    click(&mut harness, "saved-query-save-retry-reopen");
    finish_reads(&mut harness);
    assert_old_graph(&harness, &tabs);
    assert!(
        harness
            .state()
            .editing
            .database_options
            .as_ref()
            .unwrap()
            .terminal
            .is_some_and(|outcome| outcome
                .database_options_reopen_facts()
                .is_ok_and(|facts| facts.replacement))
    );
    assert!(matches!(
        harness.state().state.database_options_save_status(),
        Some((actual, Some(EditSaveOutcome::Saved(_)))) if actual == identity
    ));
    click(&mut harness, "database-options-return-settings");
    assert!(harness.state().editing.database_options.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(harness.state().state.options_dialog_open());
    assert!(harness.state().state.database_options_source_stale());
    assert_old_graph(&harness, &tabs);
    assert!(!harness.state().state.can_disable_name_auto_correct());
    assert!(!harness.state().state.can_create_table());
    assert!(!harness.state().state.can_create_table_design());
    assert!(
        harness
            .state()
            .state
            .database_options_save_status()
            .is_some()
    );
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(!journal_path(&path).exists());
    click(&mut harness, "database-options-hide-details");
    assert!(harness.state().state.options_dialog_open());
    assert!(
        harness
            .state()
            .state
            .database_options_save_status()
            .is_some()
    );
    assert_old_graph(&harness, &tabs);
    click(&mut harness, "backstage-close");
    assert!(!harness.state().state.options_dialog_open());
    assert!(harness.state().state.native_source_stale());
    harness.state_mut().open_request(Command::Open {
        path: path.clone(),
        password: String::new(),
    });
    finish_reads(&mut harness);
    assert!(!harness.state().state.native_source_stale());
    assert!(
        harness
            .state()
            .state
            .database_options_save_status()
            .is_none()
    );
    assert_eq!(std::fs::read(path).unwrap(), changed);
}
