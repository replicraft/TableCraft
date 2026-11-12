//! Actual SQL UNIQUE review, safe refusals, DROP and formerly conflicting writes.
use super::super::{cancel_refused_action, finish_refused_action};
use super::{assert_image, finish_reads, finish_save, opened, reviewed, run};
use crate::workspace::Desktop;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::path::Path;
use tablecraft_runtime::{SaveOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    inserting, unique_workflow as raw,
};

#[derive(Clone, Copy)]
enum Effect {
    Schema(SaveSchemaChange),
    Rows {
        matched: Option<usize>,
        changed: usize,
    },
}

fn decide(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    sql: &str,
    before: &[u8],
    expected: &[u8],
    effect: Effect,
) {
    let tab = harness.state().state.active_object().unwrap();
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let discarded = reviewed(harness, sql);
    if matches!(effect, Effect::Schema(SaveSchemaChange::IndexCreated)) {
        for key in [
            "query-action-schema-index-unique",
            "query-action-schema-index-unique-confirmation",
        ] {
            let label = harness.state().state.localizer.text(key);
            assert!(
                harness
                    .query_by_role_and_label(Role::Label, &label)
                    .is_some()
            );
        }
    }
    assert_image(&std::fs::read(path).unwrap(), before);
    assert!(!journal_path(path).exists());
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().apply_query_action(discarded);
    assert!(!harness.state().editing.has_save());
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
    assert_image(&std::fs::read(path).unwrap(), before);

    reviewed(harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("actual unique lifecycle save");
    };
    match effect {
        Effect::Schema(change) => {
            assert_eq!(receipt.schema_change, Some(change));
            assert!(receipt.matched_rows.is_none());
            assert!(receipt.changed_rows.is_none());
        }
        Effect::Rows { matched, changed } => {
            assert!(receipt.schema_change.is_none());
            assert_eq!(receipt.matched_rows, matched);
            assert_eq!(receipt.changed_rows, Some(changed));
        }
    }
    assert_image(&std::fs::read(path).unwrap(), expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.native_source_stale());
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        tab.id,
        "SQL tab after acknowledged operation: {sql}"
    );
    assert_eq!(harness.state().state.sql, sql);
    assert!(
        harness.state().state.query_data.is_none(),
        "save does not replay SQL"
    );
    assert!(!journal_path(path).exists());
    run(
        harness,
        "SELECT CopiedScore FROM UniqueValues ORDER BY CopiedScore",
    );
    assert_image(&std::fs::read(path).unwrap(), expected);
}

fn refuse(harness: &mut Harness<'_, Desktop>, path: &Path, sql: &str, before: &[u8]) {
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_refused_action(harness);
    assert!(!harness.state().editing.has_save());
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
    assert_image(&std::fs::read(path).unwrap(), before);
    assert!(!journal_path(path).exists());
    cancel_refused_action(harness);
    run(
        harness,
        "SELECT CopiedScore FROM UniqueValues ORDER BY CopiedScore",
    );
    assert_image(&std::fs::read(path).unwrap(), before);
}

#[test]
fn unique_create_writes_and_drop_use_real_no_yes_then_allow_the_same_previous_conflict() {
    for descending in [false, true] {
        let case = raw::source(descending);
        let source = &case.updated.creation;
        let (_directory, path, _original, mut harness) = opened(source.before.to_vec());
        let sql = format!(
            "CREATE UNIQUE INDEX ValueUnique ON UniqueValues(CopiedScore {})",
            if descending { "DESC" } else { "ASC" }
        );
        decide(
            &mut harness,
            &path,
            &sql,
            &source.before,
            &source.expected,
            Effect::Schema(SaveSchemaChange::IndexCreated),
        );
        decide(
            &mut harness,
            &path,
            "UPDATE UniqueValues SET CopiedScore=CopiedScore+10",
            &source.expected,
            &case.updated.expected,
            Effect::Rows {
                matched: Some(3),
                changed: 3,
            },
        );
        decide(
            &mut harness,
            &path,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(14)",
            &case.updated.expected,
            &case.inserted,
            Effect::Rows {
                matched: None,
                changed: 1,
            },
        );
        let first_null = inserting::after(&case.updated, &[Some(14), None]);
        decide(
            &mut harness,
            &path,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(NULL)",
            &case.inserted,
            &first_null,
            Effect::Rows {
                matched: None,
                changed: 1,
            },
        );
        decide(
            &mut harness,
            &path,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(NULL)",
            &first_null,
            &case.nulls,
            Effect::Rows {
                matched: None,
                changed: 1,
            },
        );
        // Runtime separately supplies a real Omitted input for the same native NULL.
        // Here the third NULL remains an explicit SQL NULL, not invented default input.
        decide(
            &mut harness,
            &path,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(NULL)",
            &case.nulls,
            &case.omitted,
            Effect::Rows {
                matched: None,
                changed: 1,
            },
        );
        for sql in [
            "INSERT INTO UniqueValues(CopiedScore) VALUES(12)",
            "UPDATE UniqueValues SET CopiedScore=12 WHERE CopiedScore=13",
            "INSERT INTO UniqueValues(CopiedScore) SELECT 15 UNION ALL SELECT 12",
        ] {
            refuse(&mut harness, &path, sql, &case.omitted);
        }
        decide(
            &mut harness,
            &path,
            "DROP INDEX ValueUnique ON UniqueValues",
            &case.omitted,
            &case.dropped,
            Effect::Schema(SaveSchemaChange::IndexDropped),
        );
        decide(
            &mut harness,
            &path,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(12)",
            &case.dropped,
            &case.duplicate_inserted,
            Effect::Rows {
                matched: None,
                changed: 1,
            },
        );
        decide(
            &mut harness,
            &path,
            "UPDATE UniqueValues SET CopiedScore=12 WHERE CopiedScore=11",
            &case.duplicate_inserted,
            &case.duplicate_updated,
            Effect::Rows {
                matched: Some(1),
                changed: 1,
            },
        );
        assert_eq!(
            harness
                .state()
                .state
                .query_data
                .as_ref()
                .unwrap()
                .rows
                .len(),
            8
        );
    }
}

#[test]
fn duplicate_existing_records_refuse_unique_creation_without_publishing_any_native_prefix() {
    let before = raw::duplicate_before();
    let (_directory, path, _original, mut harness) = opened(before.to_vec());
    refuse(
        &mut harness,
        &path,
        "CREATE UNIQUE INDEX ValueUnique ON UniqueValues(CopiedScore)",
        &before,
    );
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .len(),
        3
    );
}
