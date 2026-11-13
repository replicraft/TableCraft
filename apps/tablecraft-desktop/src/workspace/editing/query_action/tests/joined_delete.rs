//! Normal SQL DELETE decisions bind one original target despite duplicate join rows.
use super::{
    assert_image, cancel_refused_action, finish_reads, finish_refused_action, finish_save,
    journal_path, opened, reviewed, run,
};
use crate::workspace::test_support::objects::discard_query_close_for_open;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::{Command, SaveOutcome};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{deletes, joined_update};
use tablecraft_ui::{FluentArgs, ObjectKind};

const DELETE: &str = "DELETE DISTINCTROW t.* FROM Table1 AS t INNER JOIN Changes AS c ON t.ID=1";

#[test]
fn normal_run_joined_delete_no_then_yes_removes_one_original_target_and_reopens_exact_image() {
    let source = joined_update::source();
    let expected = deletes::after(&source.bytes, source.target_definition, &[0]);
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

    let first_review = reviewed(&mut harness, DELETE);
    let mut arguments = FluentArgs::new();
    arguments.set("count", "1");
    for key in ["query-action-matched", "query-action-delete-deleted"] {
        let label = harness.state().state.localizer.format(key, &arguments);
        harness.get_by_role_and_label(Role::Label, &label);
    }
    let warning = harness.state().state.localizer.text("query-action-no-undo");
    harness.get_by_role_and_label(Role::Label, &warning);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());

    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().apply_query_action(first_review);
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.query_action_review.is_none());
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert_eq!(harness.state().state.sql, DELETE);
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

    reviewed(&mut harness, DELETE);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)
        && receipt.schema_change.is_none()),
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
    assert_eq!(harness.state().state.sql, DELETE);
    let fresh = harness.state().state.active_object().unwrap();
    assert_eq!(fresh.target.kind, ObjectKind::Sql);
    assert!(fresh.target.document_revision > original.target.document_revision);
    assert!(!harness.state_mut().state.activate_object(&original));
    assert!(
        harness.state().state.query_data.is_none(),
        "acknowledgement does not rerun DELETE or invent query results"
    );

    run(&mut harness, "SELECT ID,Score FROM Table1 ORDER BY ID");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "2");
    assert_eq!(rows[0][1].text, "NULL");
    run(&mut harness, "SELECT Id,Score FROM Changes ORDER BY Id");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1].text, "5");
    assert_eq!(rows[1][1].text, "7");
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn changed_sql_or_reopened_source_revision_cannot_confirm_an_old_joined_delete() {
    for reopen in [false, true] {
        let source = joined_update::source();
        let (_directory, path, original, mut harness) = opened(source.bytes);
        let id = reviewed(&mut harness, DELETE);
        if reopen {
            let revision = harness
                .state()
                .state
                .database
                .as_ref()
                .unwrap()
                .document_revision;
            let generation = harness.state().generation;
            // NO_CLONE: This attempted Open and the later confirmed Open each
            // own a path while the independent file observer remains available.
            harness.state_mut().open_request(Command::Open {
                path: path.clone(),
                password: String::new(),
            });
            assert_eq!(harness.state().generation, generation);
            assert!(harness.state().state.query_action_dialog_open());
            assert!(harness.state().deferred_draft_action.is_none());
            assert!(!harness.state().editing.has_save());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            // A modal action decision must be resolved before a global Open can
            // ask to discard the still-unsaved SQL. Retain the old issued ID.
            harness
                .get_by_role_and_label(Role::Button, "No")
                .click_accesskit();
            harness.run_steps(2);
            assert!(!harness.state().state.query_action_dialog_open());
            // NO_CLONE: The accepted global Open owns its selected path; the
            // test keeps the same file to verify that no stale action can write.
            harness.state_mut().open_request(Command::Open {
                path: path.clone(),
                password: String::new(),
            });
            assert_eq!(harness.state().generation, generation);
            discard_query_close_for_open(&mut harness);
            finish_reads(&mut harness);
            assert!(harness.state().generation > generation);
            assert!(
                harness
                    .state()
                    .state
                    .database
                    .as_ref()
                    .unwrap()
                    .document_revision
                    > revision
            );
        } else {
            assert!(
                harness
                    .state_mut()
                    .state
                    .replace_query_sql("SELECT 2".into())
            );
            harness.run_steps(2);
        }

        harness.state_mut().apply_query_action(id);
        harness.run_steps(2);
        assert!(!harness.state().editing.has_save());
        assert_image(&std::fs::read(&path).unwrap(), &original);
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn late_eager_joined_delete_failure_keeps_prior_relation_and_never_stages_a_save() {
    let source = joined_update::source();
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
    let sql = "DELETE DISTINCTROW t.* FROM Table1 AS t INNER JOIN Changes AS c ON t.ID=1 WHERE c.Id=1 OR 10 \\ (c.Score-7)>0";
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

    cancel_refused_action(&mut harness);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    run(&mut harness, "SELECT Id,Score FROM Changes ORDER BY Id");
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .len(),
        2
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
}
