//! Normal SQL Run reaches manual indexed INSERT after actual CREATE and UPDATE.
mod select;

use super::{assert_image, create, finish_reads, finish_save, journal_path, opened, reviewed, run};
use crate::workspace::Desktop;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::secondary_index::normal::inserting;

fn apply(harness: &mut Harness<'_, Desktop>, sql: &str, expected: &[u8], path: &std::path::Path) {
    reviewed(harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.schema_change.is_none())
    );
    assert_image(&std::fs::read(path).unwrap(), expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.native_source_stale());
    assert_eq!(harness.state().state.sql, sql);
    assert!(
        harness.state().state.query_data.is_none(),
        "save does not rerun SQL"
    );
    assert!(!journal_path(path).exists());
}

fn decide_insert(
    harness: &mut Harness<'_, Desktop>,
    sql: &str,
    before: &[u8],
    expected: &[u8],
    path: &std::path::Path,
) {
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let discarded = reviewed(harness, sql);
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
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows.is_none() && receipt.changed_rows == Some(1)
        && receipt.schema_change.is_none()));
    assert_image(&std::fs::read(path).unwrap(), expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.native_source_stale());
    assert_eq!(harness.state().state.sql, sql);
    assert!(harness.state().state.query_data.is_none());
    assert!(!journal_path(path).exists());
}

#[test]
fn created_updated_manual_index_inserts_through_normal_no_yes_save_and_reopen() {
    for descending in [false, true] {
        for value in [None, Some(77)] {
            let case = inserting::source(descending);
            let source = &case.updated.creation;
            let sql = format!(
                "CREATE INDEX [{}] ON [{}] ([{}] {})",
                source.index_name,
                source.table,
                source.column,
                if descending { "DESC" } else { "ASC" }
            );
            let (_directory, path, _original, mut harness) = opened(source.before.to_vec());
            create(&mut harness, &sql, &source.expected, &path);
            let sql = format!(
                "UPDATE [{}] SET [{}]=CLng(Nz([{}],0)+1)",
                source.table, source.column, source.column
            );
            apply(&mut harness, &sql, &case.updated.expected, &path);
            let select = format!(
                "SELECT [{}] FROM [{}] ORDER BY [{}]",
                source.column, source.table, source.column
            );
            run(&mut harness, &select);
            let expected = inserting::after(&case.updated, &[value]);
            let sql = format!(
                "INSERT INTO [{}] ([{}]) VALUES ({})",
                source.table,
                source.column,
                value.map_or_else(|| "NULL".into(), |value| value.to_string())
            );
            decide_insert(&mut harness, &sql, &case.updated.expected, &expected, &path);
            run(&mut harness, &select);
            let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
            assert_eq!(rows.len(), source.rows.len() + 1);
            assert_image(&std::fs::read(&path).unwrap(), &expected);
        }
    }
}

#[test]
fn manual_insert_keeps_compressed_guard_and_mixed_scalar_values_after_real_prior_saves() {
    for descending in [false, true] {
        let case = inserting::compressed::source(descending);
        let source = &case.creation;
        let sql = format!(
            "CREATE INDEX [{}] ON [{}] ([{}] {})",
            source.index_name,
            source.table,
            source.column,
            if descending { "DESC" } else { "ASC" }
        );
        let (_directory, path, _original, mut harness) = opened(source.before.to_vec());
        create(&mut harness, &sql, &source.expected, &path);
        let sql = format!("UPDATE [{}] SET Guard=5", source.table);
        apply(&mut harness, &sql, &case.updated, &path);
        let select = format!(
            "SELECT KeyValue,Guard,Amount,Label FROM [{}] ORDER BY KeyValue",
            source.table
        );
        run(&mut harness, &select);
        let sql = format!(
            "INSERT INTO [{}] (KeyValue,Guard,Amount,Label) VALUES (99,NULL,CCur(1.2345),'新🦀')",
            source.table
        );
        decide_insert(&mut harness, &sql, &case.updated, &case.expected, &path);
        run(&mut harness, &select);
        let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
        let row = rows.iter().find(|row| row[0].text == "99").unwrap();
        assert_eq!(row[1].text, "NULL");
        assert_eq!(row[3].text, "新🦀");
        assert_image(&std::fs::read(&path).unwrap(), &case.expected);
    }
}
