//! Normal SQL Run reviews joined values before one existing native save actor.
use super::{Desktop, assert_image, finish_reads, finish_save, journal_path, opened, reviewed};
use crate::workspace::test_support::app_harness;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::time::{Duration, Instant};
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update,
};
use tablecraft_ui::{ObjectKind, ObjectTarget};

const UPDATE: &str =
    "UPDATE Table1 AS t INNER JOIN Changes AS c ON t.ID=c.Id SET t.Score=c.Score+20 WHERE t.ID=1";

#[test]
fn normal_run_joined_review_no_then_yes_saves_exact_image_and_restores_sql_draft() {
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
    assert!(
        !journal_path(&path).exists(),
        "review holds no save journal"
    );
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1) && receipt.schema_change.is_none()),
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
    assert!(fresh.target.document_revision > original.target.document_revision);
    assert!(!harness.state_mut().state.activate_object(&original));
    assert!(
        harness.state().state.query_data.is_none(),
        "reopen does not reexecute the action or fabricate query rows"
    );
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    let revision = fresh
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    fresh.state_mut().open_object(ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::Sql,
    });
    fresh.run_steps(2);
    super::run(&mut fresh, "SELECT ID,Score FROM Table1 ORDER BY ID");
    let data = fresh.state().state.query_data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "25");
    assert_eq!(data.rows[1][1].text, "NULL");
    super::run(&mut fresh, "SELECT Score FROM Changes ORDER BY Id");
    assert_eq!(
        fresh
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .map(|row| row[0].text.as_str())
            .collect::<Vec<_>>(),
        ["5", "7"]
    );
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn normal_run_joined_late_error_and_repeated_targets_keep_prior_results_without_writes() {
    for sql in [
        "UPDATE Table1 AS t INNER JOIN Changes AS c ON t.ID=c.Id SET t.Score=10 \\ (c.Score-7)",
        "UPDATE Table1 AS t INNER JOIN Changes AS c ON t.ID=1 SET t.Score=c.Score",
    ] {
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
        assert!(harness.state_mut().state.replace_query_sql(sql.into()));
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Run query")
            .click_accesskit();
        harness.run_steps(1);
        let deadline = Instant::now() + Duration::from_secs(10);
        while harness.state().state.busy {
            assert!(
                Instant::now() < deadline,
                "joined action refusal did not finish"
            );
            harness.run_steps(1);
            std::thread::yield_now();
        }
        harness.run_steps(2);
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
    }
}
