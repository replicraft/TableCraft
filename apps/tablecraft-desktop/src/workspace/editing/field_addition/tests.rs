//! Real headless saved-table widgets through Number review, actor and witness.
mod declaration;
mod historical_updates;
mod populated;
mod repeated;
use super::*;
use crate::workspace::test_support::{self, finish_reads};
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::{Duration, Instant};
use tablecraft_builders::table::{FieldAdditionDeclaration, FieldAdditionKind};
use tablecraft_runtime::SaveWorker;
use tablecraft_testkit::fixtures::access::{canonical_created_number as raw, canonical_creation};
use tablecraft_ui::{
    ObjectTarget,
    preview::{Preview, Scene},
};

#[test]
fn number_header_save_checks_after_and_preserves_tab_ids_order_and_scratch_sql() {
    let before = raw::source(raw::Source::IdOnly);
    let after = raw::append(&before, raw::Source::IdOnly, "Score");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    // This is the ordinary Desktop::new path, not a test capability override.
    // Initial page publication must perform the native source inspection itself.
    assert_eq!(
        harness.state().state.field_addition_available,
        SaveWorker::supports_field_addition()
    );
    assert!(harness.state().state.field_addition_available);
    let table = harness.state().state.active_object().unwrap();
    assert_eq!(
        harness
            .state()
            .state
            .field_addition_inspection(&table)
            .unwrap()
            .1,
        Some(true)
    );
    let revision = table.target.document_revision;
    assert!(
        harness
            .state()
            .state
            .can_name_field_addition(&table, FieldAdditionKind::Number)
    );
    harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    let exact_sql = "  SELECT 17 AS [Scratch]; -- keep exact user text\n";
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql(exact_sql.into())
    );
    harness.state_mut().activate_object(table.clone());
    finish_reads(&mut harness);
    let old_tabs = harness.state().state.object_tabs();
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Number")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text("Score");
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.field_addition.is_some() {
        assert!(
            Instant::now() < deadline,
            "Number actor/witness did not finish; busy={}, host_pending={}, report={:?}, status={:?}, error={:?}",
            harness.state().state.busy,
            harness.state().editing.has_save(),
            harness.state().editing.save_report(),
            harness
                .state()
                .state
                .field_addition_save_status()
                .map(|(identity, _)| (
                    identity,
                    harness
                        .state()
                        .state
                        .can_acknowledge_field_addition_save(identity),
                    harness
                        .state()
                        .state
                        .can_retry_field_addition_reopen(identity),
                )),
            harness.state().state.error
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    finish_reads(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().state.field_addition_open());
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.sql, exact_sql);
    let fresh = harness.state().state.object_tabs();
    assert_eq!(fresh.len(), old_tabs.len());
    for (old, new) in old_tabs.iter().zip(&fresh) {
        assert_eq!(old.id, new.id);
        assert_eq!(old.target.kind, new.target.kind);
        assert_ne!(old.target.document_revision, new.target.document_revision);
    }
    assert_eq!(harness.state().state.active_object().unwrap().id, table.id);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.last().unwrap().name, "Score");
    assert!(page.rows.is_empty());
    assert_eq!(
        page.source_revision,
        Some(fresh[0].target.document_revision)
    );
}

#[test]
fn review_cancellation_latch_never_starts_actor_even_if_capability_finishes_first() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    harness.state_mut().state.field_addition_available = true;
    harness.state_mut().inspect_field_addition_availability();
    finish_reads(&mut harness);
    let tab = harness.state().state.active_object().unwrap();
    assert!(harness.state_mut().state.begin_field_addition_name(
        &tab,
        "Score".into(),
        FieldAdditionDeclaration::NumberLongZero
    ));
    let request = harness
        .state()
        .state
        .field_addition_review_request()
        .unwrap();
    harness.state_mut().save_field_addition(request);
    let id = harness
        .state()
        .editing
        .field_addition
        .as_ref()
        .unwrap()
        .request_id;
    harness.state_mut().cancel_field_addition_review(id);
    assert!(!harness.state().worker.publication_is_current(id));
    assert!(
        !harness.state().state.busy,
        "revoked readonly replies cannot finish this owner later"
    );
    harness.run_steps(2);
    assert!(harness.state().editing.field_addition.is_none());
    assert!(harness.state().editing.save.is_none());
    assert_eq!(
        harness.state().state.field_addition_name().unwrap().1,
        "Score"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
}

mod preflight;

#[test]
fn normal_native_host_still_requires_positive_source_while_mock_has_no_write_support() {
    let before = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    let (_directory, path, _, harness) = test_support::opened(before.clone());
    assert!(harness.state().state.field_addition_available);
    assert_eq!(
        harness.state().state.field_addition_available,
        SaveWorker::supports_field_addition()
    );
    let table = harness.state().state.active_object().unwrap();
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    assert_eq!(
        harness
            .state()
            .state
            .field_addition_inspection(&table)
            .unwrap()
            .1,
        Some(false)
    );
    assert!(
        !harness
            .state()
            .state
            .can_name_field_addition(&table, FieldAdditionKind::Number)
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Click to Add")
            .accesskit_node()
            .is_disabled()
    );
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(path).unwrap(), before);
    let mock = Preview::new(Scene::Datasheet);
    assert!(!mock.state().field_addition_available);
    assert!(mock.state().active_object().is_some());
    assert!(!mock.state().can_name_field_addition(
        &mock.state().active_object().unwrap(),
        FieldAdditionKind::Number
    ));
}

mod after_off;

mod after_off_history;

mod design;
