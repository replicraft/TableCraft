//! Real actor/ACK/read boundaries; no product pause flags or substituted outcomes.
mod cancellation;
use crate::workspace::{
    Desktop,
    test_support::{self, finish_reads},
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_builders::table::FieldAdditionDeclaration;
use tablecraft_runtime::{
    Command, FieldAdditionReview, Response, ReviewedFieldAddition, SaveOutcome, SaveReport,
    journal_path,
};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;
use tablecraft_ui::{EditSaveOutcome, FieldFilterTarget, SortSpec};

const FIXTURE_PAGE_BYTES: usize = 4096;
// The independent raw fixture places its LVAL body at this page's tail. This
// byte is in its unused gap, so the whole-image witness changes without changing
// the saved field declaration or making the later ordinary read malformed.
const FIXTURE_UNUSED_PROPERTY_BYTE: usize = 10 * FIXTURE_PAGE_BYTES + 128;

fn click(harness: &mut Harness<'_, Desktop>, label: &str) {
    harness
        .get_all_by_role_and_label(Role::Button, label)
        .find(|node| !node.accesskit_node().is_disabled())
        .expect("enabled action")
        .click_accesskit();
    harness.run_steps(1);
}

fn take_review(
    harness: &Harness<'_, Desktop>,
) -> (u64, FieldAdditionReview, ReviewedFieldAddition) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((generation, result)) = harness.state().worker.poll() {
            let Response::FieldAdditionReviewed { review, capability } = result.unwrap() else {
                panic!("field native review expected");
            };
            return (generation, review, capability);
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

fn take_report(harness: &Harness<'_, Desktop>) -> SaveReport {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(report) = harness.state().editing.worker.poll() {
            return report;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

fn enable(harness: &mut Harness<'_, Desktop>) {
    harness.state_mut().state.field_addition_available = true;
    harness.state_mut().inspect_field_addition_availability();
    finish_reads(harness);
}

#[test]
fn saved_after_tamper_can_return_discard_and_reopen_without_rollback_or_write_replay() {
    let before = raw::source(raw::Source::IdOnly);
    let after = raw::append(&before, raw::Source::IdOnly, "Score");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    enable(&mut harness);
    let table = harness.state().state.active_object().unwrap();
    assert!(harness.state_mut().state.begin_field_addition_name(
        &table,
        "Score".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    // The original file transition remains pending through the failed read.
    harness.state_mut().open_request(Command::Open {
        path: path.clone(),
        password: String::new(),
    });
    harness.run_steps(2);
    click(&mut harness, "Save");
    let (generation, review, capability) = take_review(&harness);
    harness
        .state_mut()
        .start_reviewed_field_addition(generation, review, capability);
    let identity = harness
        .state()
        .editing
        .field_addition
        .as_ref()
        .unwrap()
        .identity
        .unwrap();
    let report = take_report(&harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    // SaveWorker reports remain available until ACK. Invoke the actual host
    // cancellation path before any more frames so ordinary auto-ACK is latched
    // off, then publish through the normal retained-report controller.
    harness.state_mut().cancel_save(identity.save_id);
    harness.state_mut().poll_saves();
    assert!(!harness.state_mut().return_to_field_addition_draft(identity));
    assert!(
        harness
            .state_mut()
            .acknowledge_field_addition_actor(identity)
    );
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert_eq!(std::fs::read(&path).unwrap(), after);
    let cleanup = harness
        .state()
        .editing
        .field_addition
        .as_ref()
        .unwrap()
        .cleanup;
    let mut tampered = after;
    assert_eq!(tampered[FIXTURE_UNUSED_PROPERTY_BYTE], 0);
    tampered[FIXTURE_UNUSED_PROPERTY_BYTE] = 1;
    // The real ACK has released native handles before this external mutation.
    std::fs::write(&path, &tampered).unwrap();
    harness.state_mut().reopen_field_addition(identity);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness
        .state()
        .editing
        .field_addition
        .as_ref()
        .unwrap()
        .reopening
    {
        assert!(Instant::now() < deadline);
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(
        harness
            .state()
            .state
            .can_return_to_field_addition_draft(identity)
    );
    assert!(harness.state().deferred_draft_action.is_some());
    assert!(matches!(
        harness
            .state()
            .state
            .field_addition_save_status()
            .unwrap()
            .1,
        Some(EditSaveOutcome::Saved(_))
    ));
    harness.run_steps(2);
    click(&mut harness, "Return to draft");
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().deferred_draft_action.is_none());
    assert!(harness.state().state.native_mutation_blocked());
    assert_eq!(
        harness.state().state.field_addition_name().unwrap().1,
        "Score"
    );
    let warning = harness
        .state()
        .state
        .field_addition_saved_warning()
        .unwrap();
    assert_eq!(warning.identity, identity);
    assert!(matches!(warning.outcome, Some(EditSaveOutcome::Saved(_))));
    assert!(
        warning
            .reopen_error
            .is_some_and(|message| !message.is_empty())
    );
    assert_eq!(warning.cleanup, cleanup);
    assert!(!harness.state().state.can_review_field_addition());
    // Exercise the actual Backstage-visible File/Open path, not just direct
    // datasheet admission. Discard closes presentation and never undoes save.
    harness.state_mut().state.show_open();
    harness.run_steps(2);
    assert!(harness.state().state.backstage_visible());
    harness.state_mut().open_request(Command::Open {
        path: path.clone(),
        password: String::new(),
    });
    harness.run_steps(2);
    assert!(harness.state().state.field_addition_close_open());
    assert!(!harness.state().state.can_save_field_addition_close());
    click(&mut harness, "Close draft");
    finish_reads(&mut harness);
    assert!(!harness.state().state.native_mutation_blocked());
    assert!(
        harness
            .state()
            .state
            .field_addition_saved_warning()
            .is_none()
    );
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .columns
            .last()
            .unwrap()
            .name,
        "Score"
    );
    assert_eq!(std::fs::read(&path).unwrap(), tampered);
    assert!(!journal_path(&path).exists());
}

#[test]
fn checked_number_save_revalidates_filter_and_sort_preferences_against_fresh_schema() {
    let before = raw::source(raw::Source::IdOnly);
    let after = raw::append(&before, raw::Source::IdOnly, "Score");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    enable(&mut harness);
    let sort = SortSpec {
        column: "ID".into(),
        descending: true,
    };
    let target = harness.state().state.current_sort_target().unwrap();
    harness
        .state_mut()
        .sort_view(target, Some(sort.clone()), false);
    finish_reads(&mut harness);
    let filter = FieldFilterTarget {
        document_revision: harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        table: "Table1".into(),
        column: "ID".into(),
        filter_id: harness
            .state()
            .state
            .active_filter_target()
            .map(|target| target.filter_id),
        enabled: harness.state().state.table_filter_enabled(),
    };
    harness.state_mut().describe_field_filter(filter.clone());
    finish_reads(&mut harness);
    // The field controller accepts only the operator and inputs retained by its
    // real widgets. Set both through those controls before dispatching apply.
    harness
        .get_by_role_and_label(Role::ComboBox, "Comparison")
        .click_accesskit();
    harness.run_steps(2);
    click(&mut harness, "Greater than");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Value")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Value")
        .type_text("0");
    harness.run_steps(2);
    click(&mut harness, "Apply filter");
    finish_reads(&mut harness);
    if let Some(id) = harness.state().state.field_filter_request_id() {
        harness.state_mut().close_field_filter(id);
    }
    assert_eq!(harness.state().state.table_sort(), Some(sort.clone()));
    assert_eq!(harness.state().state.table_filter_count(), 1);
    assert!(harness.state().state.table_filter_enabled());
    let old_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    let table = harness.state().state.active_object().unwrap();
    assert!(harness.state_mut().state.begin_field_addition_name(
        &table,
        "Score".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    let request = harness
        .state()
        .state
        .field_addition_review_request()
        .unwrap();
    harness.state_mut().save_field_addition(request);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.has_save() {
        assert!(
            Instant::now() < deadline,
            "Number save/filter restoration did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.table_sort(), Some(sort));
    assert_eq!(harness.state().state.table_filter_count(), 1);
    assert!(harness.state().state.table_filter_enabled());
    let handle = harness.state().filter.as_ref().unwrap();
    assert_ne!(handle.target().document_revision, old_revision);
    assert_eq!(
        handle.target().document_revision,
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision
    );
    assert_eq!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .columns
            .last()
            .unwrap()
            .name,
        "Score"
    );
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), after);
}
