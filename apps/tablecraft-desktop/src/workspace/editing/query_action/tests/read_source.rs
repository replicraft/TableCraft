//! Normal SQL Run uses the existing issued review and save/acknowledgment actor.
use super::{
    assert_image, finish_reads, finish_refused_action, finish_save, journal_path, opened, reviewed,
    run,
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update,
};
use tablecraft_ui::ObjectKind;

const UPDATE: &str = "UPDATE Table1 AS t SET t.Score=25 WHERE EXISTS (SELECT c.Id FROM Changes AS c WHERE c.Id=t.ID AND c.Score=5)";

#[test]
fn normal_run_correlated_update_reviews_then_saves_and_reopens_exact_native_values() {
    let source = joined_update::source();
    let expected =
        field_changes::update_score(&source.bytes, source.target_definition, 0, Some(25));
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let original = harness.state().state.active_object().unwrap();
    let prior = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    reviewed(&mut harness, UPDATE);
    let warning = harness.state().state.localizer.text("query-action-no-undo");
    harness.get_by_role_and_label(Role::Label, &warning);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        prior
    );
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.query_action_review.is_none());
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert_eq!(harness.state().state.sql, UPDATE);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        prior
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    reviewed(&mut harness, UPDATE);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1) && receipt.schema_change.is_none()),
        "{report:?}"
    );
    assert!(harness.state().editing.has_save());
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());
    assert_eq!(harness.state().state.sql, UPDATE);
    let fresh = harness.state().state.active_object().unwrap();
    assert_eq!(fresh.target.kind, ObjectKind::Sql);
    assert_ne!(fresh, original);
    assert!(
        harness.state().state.query_data.is_none(),
        "reopen does not rerun SQL"
    );
    run(&mut harness, "SELECT ID,Score FROM Table1 ORDER BY ID");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1].text, "25");
    assert_eq!(rows[1][1].text, "NULL");
    run(&mut harness, "SELECT Id,Score FROM Changes ORDER BY Id");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows[0][1].text, "5");
    assert_eq!(rows[1][1].text, "7");
    assert_image(&std::fs::read(&path).unwrap(), &expected);
}

#[test]
fn late_correlated_error_preserves_sql_tab_previous_result_and_original_file() {
    let (_directory, path, before, mut harness) = opened(joined_update::source().bytes);
    let original = harness.state().state.active_object().unwrap();
    let prior = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let sql = "UPDATE Table1 AS t SET t.Score=25 WHERE EXISTS (SELECT 10 \\ (c.Score-7) FROM Changes AS c WHERE c.Id=t.ID OR t.ID=1)";
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_refused_action(&mut harness);
    assert!(harness.state().editing.query_action_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert_eq!(harness.state().state.sql, sql);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        prior
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    super::cancel_refused_action(&mut harness);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    run(&mut harness, "SELECT Id,Score FROM Changes ORDER BY Id");
    assert!(harness.state().state.error.is_none());
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][1].text,
        "5"
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
}
