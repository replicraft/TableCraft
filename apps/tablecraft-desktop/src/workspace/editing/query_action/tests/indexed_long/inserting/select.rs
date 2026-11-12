//! Normal SQL Run preserves complete INSERT SELECT staging and consuming review.
use super::super::{
    assert_image, cancel_refused_action, create, finish_reads, finish_refused_action, finish_save,
    journal_path, opened, reviewed, run,
};
use super::{apply, decide_insert};
use crate::workspace::Desktop;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::path::PathBuf;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::secondary_index::normal::inserting as raw;

fn prepared(
    descending: bool,
) -> (
    tempfile::TempDir,
    PathBuf,
    Harness<'static, Desktop>,
    raw::Case,
) {
    let case = raw::source(descending);
    let source = &case.updated.creation;
    let (directory, path, _before, mut harness) = opened(source.before.to_vec());
    let sql = format!(
        "CREATE INDEX [{}] ON [{}] ([{}] {})",
        source.index_name,
        source.table,
        source.column,
        if descending { "DESC" } else { "ASC" }
    );
    create(&mut harness, &sql, &source.expected, &path);
    let sql = format!(
        "UPDATE [{}] SET [{}]=CLng(Nz([{}],0)+1)",
        source.table, source.column, source.column
    );
    apply(&mut harness, &sql, &case.updated.expected, &path);
    run(
        &mut harness,
        "SELECT CopiedScore FROM Archive ORDER BY CopiedScore",
    );
    let expected = raw::after(&case.updated, &[Some(77)]);
    decide_insert(
        &mut harness,
        "INSERT INTO Archive (CopiedScore) VALUES (77)",
        &case.updated.expected,
        &expected,
        &path,
    );
    run(
        &mut harness,
        "SELECT CopiedScore FROM Archive ORDER BY CopiedScore",
    );
    (directory, path, harness, case)
}

#[test]
fn self_join_and_aggregate_insert_select_no_then_yes_publish_one_complete_native_save() {
    let cases: [(&str, &[Option<i32>]); 2] = [
        (
            "INSERT INTO Archive (CopiedScore) SELECT t.CopiedScore FROM Archive AS t INNER JOIN Archive AS c ON t.CopiedScore=c.CopiedScore ORDER BY t.CopiedScore",
            &[
                Some(-6),
                Some(-6),
                Some(-6),
                Some(-6),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(8),
                Some(77),
            ],
        ),
        (
            "INSERT INTO Archive (CopiedScore) SELECT CLng(Sum(CopiedScore)) FROM Archive",
            &[Some(76)],
        ),
    ];
    for descending in [false, true] {
        for (sql, selected) in cases {
            let (_directory, path, mut harness, case) = prepared(descending);
            let before = raw::after(&case.updated, &[Some(77)]);
            let previous = harness
                .state()
                .state
                .query_data
                .as_ref()
                .unwrap()
                .rows
                .as_ptr();
            let discarded = reviewed(&mut harness, sql);
            assert_image(&std::fs::read(&path).unwrap(), &before);
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
            assert_image(&std::fs::read(&path).unwrap(), &before);
            assert!(!journal_path(&path).exists());

            reviewed(&mut harness, sql);
            harness
                .get_by_role_and_label(Role::Button, "Yes")
                .click_accesskit();
            harness.run_steps(1);
            let report = finish_save(&mut harness);
            assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.matched_rows.is_none() && receipt.changed_rows == Some(selected.len())
                && receipt.schema_change.is_none()));
            let mut appended = vec![Some(77)];
            appended.extend_from_slice(selected);
            let expected = raw::after(&case.updated, &appended);
            assert_image(&std::fs::read(&path).unwrap(), &expected);
            harness
                .get_by_role_and_label(Role::Button, "Close")
                .click_accesskit();
            harness.run_steps(1);
            finish_reads(&mut harness);
            assert!(!harness.state().editing.has_save());
            assert!(!harness.state().state.native_source_stale());
            assert_eq!(harness.state().state.sql, sql);
            assert!(
                harness.state().state.query_data.is_none(),
                "save must not rerun INSERT SELECT"
            );
            run(
                &mut harness,
                "SELECT CopiedScore FROM Archive ORDER BY CopiedScore",
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
                case.updated.creation.rows.len() + appended.len()
            );
            assert_image(&std::fs::read(&path).unwrap(), &expected);
            assert!(!journal_path(&path).exists());
        }
    }
}

#[test]
fn top_omitted_late_source_error_keeps_previous_sql_results_and_every_original_byte() {
    let (_directory, path, mut harness, case) = prepared(false);
    let before = raw::after(&case.updated, &[Some(77)]);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let sql = r"INSERT INTO Archive (CopiedScore) SELECT TOP 1 CLng(10 \ (77-CopiedScore)) FROM Archive ORDER BY CopiedScore";
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_refused_action(&mut harness);
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
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    cancel_refused_action(&mut harness);
    run(
        &mut harness,
        "SELECT CopiedScore FROM Archive ORDER BY CopiedScore",
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
        7
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
}
