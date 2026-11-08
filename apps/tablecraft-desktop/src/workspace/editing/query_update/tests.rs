//! Connected SQL action review, decision, save and fresh reopen without windows.
use super::*;
use crate::workspace::test_support::{self, finish_reads, finish_save};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::{SaveFailureKind, SaveOutcome};
use tablecraft_testkit::fixtures::access::mutation_fixture;
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;
use tablecraft_ui::{ObjectKind, ObjectTarget};

const UPDATE: &str = "UPDATE Items SET Score=Score+7 WHERE Id>=2";

fn opened() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    Vec<u8>,
    Harness<'static, Desktop>,
) {
    let (directory, path, original, mut harness) =
        test_support::opened(mutation_fixture::database());
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness.state_mut().open_object(ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::Sql,
    });
    harness.run_steps(2);
    run(&mut harness, "SELECT 99 AS Previous");
    (directory, path, original, harness)
}

fn run(harness: &mut Harness<'_, Desktop>, sql: &str) {
    assert!(harness.state_mut().state.replace_query_sql(sql.into()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

fn reviewed(harness: &mut Harness<'_, Desktop>) -> u64 {
    run(harness, UPDATE);
    let (id, _) = harness
        .state()
        .editing
        .query_update_review
        .as_ref()
        .expect("runtime native review");
    *id
}

#[test]
fn no_preserves_previous_results_and_yes_saves_once_then_restores_sql_without_reexecution() {
    let (_directory, path, original, mut harness) = opened();
    let rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let declined = reviewed(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), original);
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
        rows
    );
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().apply_query_update(declined);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.query_update_review.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );

    reviewed(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(2) && receipt.changed_rows == Some(2)));
    assert!(harness.state().editing.has_save());
    assert!(harness.state().state.query_update_dialog_open());
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.sql, UPDATE);
    assert!(
        harness
            .state()
            .state
            .active_object()
            .is_some_and(|tab| tab.target.kind == ObjectKind::Sql)
    );
    assert!(
        harness.state().state.query_data.is_none(),
        "fresh reopen does not fabricate or rerun UPDATE results"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        mutation_fixture::with_rows(&[(1, 10), (2, 27), (3, 37)])
    );
    assert!(!journal_path(&path).exists());
    run(&mut harness, "SELECT Score FROM Items ORDER BY Id");
    let data = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(
        data.rows
            .iter()
            .map(|row| row[0].text.as_str())
            .collect::<Vec<_>>(),
        ["10", "27", "37"]
    );
}

#[test]
fn stale_source_apply_retains_resources_until_explicit_release_and_acknowledgment() {
    let (_directory, path, mut changed, mut harness) = opened();
    reviewed(&mut harness);
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Failed(failure)
        if failure.kind == SaveFailureKind::SourceChanged && failure.can_release && !failure.can_recover));
    assert_retained_target_observation(&path, &changed);
    assert!(!journal_path(&path).exists());
    assert!(harness.state().editing.has_save());
    harness.state_mut().acknowledge_save(report.id);
    assert!(harness.state().editing.has_save());
    harness
        .get_by_role_and_label(Role::Button, "Release save resources")
        .click_accesskit();
    harness.run_steps(1);
    let released = finish_save(&mut harness);
    assert!(matches!(released.outcome, SaveOutcome::Released { .. }));
    assert_ne!(released.id, report.id);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

#[test]
fn unsupported_actions_never_open_value_prompts_or_write() {
    for sql in [
        "DELETE FROM Items WHERE Id=1",
        "INSERT INTO Items (Id,Score) VALUES (4,40)",
    ] {
        let (_directory, path, original, mut harness) = opened();
        let rows = harness
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
        // Unsupported syntax intentionally surfaces a global diagnostic, so
        // this refusal case does not use finish_reads' success-only assertion.
        let deadline = Instant::now() + Duration::from_secs(10);
        while harness.state().state.busy {
            assert!(Instant::now() < deadline, "action refusal did not finish");
            harness.run_steps(1);
            std::thread::yield_now();
        }
        harness.run_steps(2);
        assert!(harness.state().editing.query_update_review.is_none());
        assert!(!harness.state().editing.has_save());
        assert!(
            harness
                .query_by_role_and_label(Role::TextInput, "Value")
                .is_none()
        );
        assert_eq!(
            harness
                .state()
                .state
                .query_data
                .as_ref()
                .unwrap()
                .rows
                .as_ptr(),
            rows
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!journal_path(&path).exists());
        if harness.state().state.query_update_dialog_open() {
            harness.key_press(Key::Escape);
            harness.run_steps(2);
        }
    }
}

#[test]
fn declared_update_prompts_precede_review_and_save_the_confirmed_values_once() {
    let (_directory, path, original, mut harness) = opened();
    let sql = "PARAMETERS [Increment] LONG, [Chosen] LONG; UPDATE Items SET Score=Score+[Increment] WHERE Id=[Chosen]";
    run(&mut harness, sql);
    assert!(harness.state().query_parameters.is_some());
    assert!(harness.state().editing.query_update_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());

    for (name, value) in [("Increment", "7"), ("Chosen", "2")] {
        harness.get_by_role_and_label(Role::TextInput, name).focus();
        harness.run_steps(1);
        harness.event(egui::Event::Text(value.into()));
        harness.run_steps(1);
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .click_accesskit();
        harness.run_steps(2);
    }
    finish_reads(&mut harness);
    assert!(harness.state().query_parameters.is_none());
    assert!(harness.state().editing.query_update_review.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );

    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.sql, sql);
    assert!(harness.state().state.query_data.is_none());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        mutation_fixture::with_rows(&[(1, 10), (2, 27), (3, 30)])
    );
    assert!(!journal_path(&path).exists());
}

#[test]
fn cancelling_an_update_parameter_prompt_preserves_results_and_never_reviews() {
    let (_directory, path, original, mut harness) = opened();
    run(
        &mut harness,
        "PARAMETERS [Value] LONG; UPDATE Items SET Score=[Value] WHERE Id=1",
    );
    assert!(harness.state().query_parameters.is_some());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().query_parameters.is_none());
    assert!(harness.state().editing.query_update_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
}
