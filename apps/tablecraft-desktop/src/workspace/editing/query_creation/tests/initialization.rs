//! First native QueryDef save through the actual Name, actor and guarded reopen.
mod editing;
use super::{accept_name, name, opened_source, run_saved, scratch, sql};
use crate::workspace::test_support::{finish_reads, finish_schema_save};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_core::values::LegacyDateTime;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::saved_queries::creation::initialization::{
    self as raw, Case,
};
use tablecraft_ui::{ObjectKind, ObjectTarget};

fn run(case: Case) {
    let at = LegacyDateTime::from_ole_days(45000.5).unwrap();
    let expected = raw::source(case, "Created", "Answer", 99, at);
    let (_directory, path, source, clock, mut harness) = opened_source(expected);
    let database = harness.state().state.database.as_ref().unwrap();
    let revision = database.document_revision;
    assert!(database.queries.is_empty());
    assert!(
        database
            .tables
            .iter()
            .all(|table| table.name != "MSysQueries")
    );
    // NO_CLONE: The old catalog observer must survive the actual mutable
    // publication replacing its UI owner; borrowing those names cannot do so.
    let prior = database
        .tables
        .iter()
        .map(|table| {
            (
                table.name.clone(),
                table.local,
                table.system,
                table.storage.clone(),
            )
        })
        .collect::<Vec<_>>();

    scratch(&mut harness);
    sql(&mut harness, "SELECT 7 AS Previous;");
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    sql(&mut harness, "SELECT 99 AS Answer;");
    let old = harness.state().state.active_object().unwrap();
    name(&mut harness, false);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.query_creation_name_open());
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&old));
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Answer;");
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
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
    assert_eq!(clock.calls(), 0);
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), source.before);
    assert!(!journal_path(&path).exists());

    name(&mut harness, false);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    let active = harness.state().state.active_object().unwrap();
    assert_eq!(active.id, old.id);
    assert!(
        matches!(&active.target.kind, ObjectKind::SavedQuery { name, .. } if name == "Created")
    );
    assert!(active.target.document_revision > revision);
    assert!(!harness.state().state.can_activate_object(&old));
    assert!(!harness.state_mut().state.activate_object(&old));
    assert_eq!(
        harness.state().state.active_object().as_ref(),
        Some(&active)
    );
    let fresh = harness.state().state.database.as_ref().unwrap();
    assert_eq!(fresh.tables.len(), prior.len() + 1);
    assert_eq!(fresh.queries.len(), 1);
    let initialized = fresh
        .tables
        .iter()
        .filter(|table| table.name == "MSysQueries")
        .collect::<Vec<_>>();
    assert_eq!(initialized.len(), 1);
    assert!(initialized[0].local && initialized[0].system);
    for (old, new) in prior.iter().zip(
        fresh
            .tables
            .iter()
            .filter(|table| table.name != "MSysQueries"),
    ) {
        assert_eq!(
            (&old.0, old.1, old.2, &old.3),
            (&new.name, new.local, new.system, &new.storage)
        );
    }
    // The independent complete oracle pins every prior native catalog ID,
    // table declaration, record and allocation byte, not just visible names.
    assert_eq!(std::fs::read(&path).unwrap(), source.expected);
    assert_eq!(clock.calls(), 1);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());
    run_saved(&mut harness, "99");

    if matches!(case, Case::Ordinary) {
        let revision = harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision;
        harness.state_mut().open_object(ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Table {
                name: "Items".into(),
            },
        });
        harness.run_steps(1);
        finish_reads(&mut harness);
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.rows.len(), 1);
        assert_eq!(data.rows[0][0].text, "1");
        assert_eq!(data.rows[0][1].text, "10");
    }
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), source.expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn first_query_in_a_real_blank_file_uses_name_save_ack_reopen_and_saved_run() {
    run(Case::Blank);
}

#[test]
fn first_query_keeps_existing_table_data_native_identities_and_old_target_revocation() {
    run(Case::Ordinary);
}
