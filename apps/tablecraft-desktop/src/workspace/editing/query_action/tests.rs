//! Connected SQL action review, decision, save and fresh reopen without windows.
mod schema;

use super::*;
use crate::workspace::test_support::{self, finish_reads, finish_save};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::{SaveFailureKind, SaveOutcome};
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_creation_defaults as defaults, canonical_native17_creation,
    mutation_fixture,
};
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;
use tablecraft_ui::{ObjectKind, ObjectTarget};

const UPDATE: &str = "UPDATE Items SET Score=Score+7 WHERE Id>=2";

fn opened(
    original: Vec<u8>,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    Vec<u8>,
    Harness<'static, Desktop>,
) {
    let (directory, path, original, mut harness) = test_support::opened(original);
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

fn finish_refused_action(harness: &mut Harness<'_, Desktop>) {
    let diagnostic = harness.state().state.localizer.text("diagnostics-copy");
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness
        .query_by_role_and_label(Role::Button, &diagnostic)
        .is_none()
        && harness.state().editing.query_action_review.is_none()
        && !harness.state().editing.has_save()
    {
        assert!(
            Instant::now() < deadline,
            "action did not publish a terminal refusal"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(
        harness
            .query_by_role_and_label(Role::Button, &diagnostic)
            .is_some(),
        "action unexpectedly produced a review or save instead of its diagnostic"
    );
    assert!(harness.state().editing.query_action_pending.is_none());
    assert!(harness.state().editing.query_action_review.is_none());
    assert!(!harness.state().editing.has_save());
}

fn cancel_refused_action(harness: &mut Harness<'_, Desktop>) {
    let original = harness.state().state.active_object();
    let sql = harness.state().state.sql.to_string();
    let rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let cancel = harness.state().state.localizer.text("shell-cancel");
    harness
        .get_by_role_and_label(Role::Button, &cancel)
        .click_accesskit();
    harness.run_steps(2);
    let diagnostic = harness.state().state.localizer.text("diagnostics-copy");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, &diagnostic)
            .is_none()
    );
    assert_eq!(harness.state().state.active_object(), original);
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
        rows
    );
    assert!(harness.state().editing.query_action_pending.is_none());
    assert!(harness.state().editing.query_action_review.is_none());
    assert!(!harness.state().editing.has_save());
}

fn reviewed(harness: &mut Harness<'_, Desktop>, sql: &str) -> u64 {
    run(harness, sql);
    let (id, _) = harness
        .state()
        .editing
        .query_action_review
        .as_ref()
        .expect("runtime native review");
    *id
}

#[test]
fn no_preserves_previous_results_and_yes_saves_once_then_restores_sql_without_reexecution() {
    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    let rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let declined = reviewed(&mut harness, UPDATE);
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
    harness.state_mut().apply_query_action(declined);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.query_action_review.is_none());
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

    reviewed(&mut harness, UPDATE);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows == Some(2) && receipt.changed_rows == Some(2)));
    assert!(harness.state().editing.has_save());
    assert!(harness.state().state.query_action_dialog_open());
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
    for (source, sql) in [
        (mutation_fixture::database(), UPDATE),
        (delete_source(), "DELETE FROM Items WHERE Id>=63"),
        (
            defaults::source(defaults::Case::Seven).bytes,
            "INSERT INTO Items (Id) VALUES (24)",
        ),
    ] {
        let (_directory, path, mut changed, mut harness) = opened(source);
        reviewed(&mut harness, sql);
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
}

#[test]
fn unsupported_actions_never_open_value_prompts_or_write() {
    for sql in [
        "DELETE FROM Items AS i, Items AS j WHERE i.Id=j.Id",
        "INSERT INTO Items (Id,Score) IN 'external.accdb' SELECT Id,Score FROM Items",
    ] {
        let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
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
        assert!(harness.state().editing.query_action_review.is_none());
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
        if harness.state().state.query_action_dialog_open() {
            harness.key_press(Key::Escape);
            harness.run_steps(2);
        }
    }
}

#[test]
fn declared_update_prompts_precede_review_and_save_the_confirmed_values_once() {
    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    let sql = "PARAMETERS [Increment] LONG, [Chosen] LONG; UPDATE Items SET Score=Score+[Increment] WHERE Id=[Chosen]";
    run(&mut harness, sql);
    assert!(harness.state().query_parameters.is_some());
    assert!(harness.state().editing.query_action_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());

    for (name, value) in [("Increment", "7"), ("Chosen", "2")] {
        confirm_parameter(&mut harness, name, value);
    }
    finish_reads(&mut harness);
    assert!(harness.state().query_parameters.is_none());
    assert!(harness.state().editing.query_action_review.is_some());
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
fn cancelling_action_parameter_prompts_preserves_results_and_never_reviews() {
    for (source, sql) in [
        (
            mutation_fixture::database(),
            "PARAMETERS [Value] LONG; UPDATE Items SET Score=[Value] WHERE Id=1",
        ),
        (
            delete_source(),
            "PARAMETERS [Value] LONG; DELETE FROM Items WHERE Id=[Value]",
        ),
        (
            defaults::source(defaults::Case::Seven).bytes,
            "PARAMETERS [Value] LONG; INSERT INTO Items (Id) VALUES ([Value])",
        ),
    ] {
        let (_directory, path, original, mut harness) = opened(source);
        run(&mut harness, sql);
        assert!(harness.state().query_parameters.is_some());
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .click_accesskit();
        harness.run_steps(2);
        assert!(harness.state().query_parameters.is_none());
        assert!(harness.state().editing.query_action_review.is_none());
        assert!(!harness.state().editing.has_save());
        assert_eq!(
            harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
            "99"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!journal_path(&path).exists());
    }
}

fn delete_source() -> Vec<u8> {
    canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    )
}

fn confirm_parameter(harness: &mut Harness<'_, Desktop>, name: &str, value: &str) {
    harness.get_by_role_and_label(Role::TextInput, name).focus();
    harness.run_steps(1);
    harness.event(egui::Event::Text(value.into()));
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(2);
}

#[test]
fn typed_delete_no_preserves_original_and_yes_commits_literal_images_without_reexecution() {
    let (_directory, path, original, mut harness) = opened(delete_source());
    let sql = "PARAMETERS [Minimum] LONG, [Maximum] LONG; DELETE FROM Items WHERE Id BETWEEN [Minimum] AND [Maximum]";
    for accept in [false, true] {
        run(&mut harness, sql);
        assert!(harness.state().query_parameters.is_some());
        assert!(harness.state().editing.query_action_review.is_none());
        for (name, value) in [("Minimum", "63"), ("Maximum", "64")] {
            confirm_parameter(&mut harness, name, value);
        }
        finish_reads(&mut harness);
        assert!(matches!(
            harness.state().editing.query_action_review.as_ref(),
            Some((_, ReviewedAction::Delete(_)))
        ));
        assert!(!harness.state().editing.has_save());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!journal_path(&path).exists());
        assert_eq!(
            harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
            "99"
        );
        harness
            .get_by_role_and_label(Role::Button, if accept { "Yes" } else { "No" })
            .click_accesskit();
        harness.run_steps(2);
        if !accept {
            assert!(harness.state().editing.query_action_review.is_none());
            assert!(!harness.state().editing.has_save());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            continue;
        }
        let report = finish_save(&mut harness);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows == Some(2) && receipt.changed_rows == Some(2)));
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
    }

    // Independent source-layout oracle: keep all payload bytes and change only
    // the two original DATA root tombstones and the definition's live count.
    let mut expected = original.clone();
    for slot in [62, 63] {
        let offset = 8 * 4096 + 14 + slot * 2;
        let word = u16::from_le_bytes(original[offset..offset + 2].try_into().unwrap());
        expected[offset..offset + 2].copy_from_slice(&(word | 0xc000).to_le_bytes());
    }
    expected[7 * 4096 + 16..7 * 4096 + 20].copy_from_slice(&62_u32.to_le_bytes());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    assert_eq!(harness.state().state.sql, sql);
    assert!(harness.state().state.query_data.is_none());
    run(&mut harness, "SELECT Count(*) AS Remaining FROM Items");
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "62"
    );
}

#[test]
fn typed_values_and_select_appends_preserve_presence_and_save_the_original_batch_once() {
    let mut cases = Vec::new();
    for (sql, value, supplied_null) in [
        (
            "PARAMETERS [Added] LONG; INSERT INTO Items (Id) VALUES ([Added])",
            "24",
            false,
        ),
        (
            "PARAMETERS [Added] LONG; INSERT INTO Items (Id,Score) VALUES ([Added],NULL)",
            "25",
            true,
        ),
    ] {
        let source = defaults::source(defaults::Case::Seven);
        let (expected, _) = defaults::inserted(&source, supplied_null).unwrap();
        cases.push((sql, value, source.bytes, expected, 1));
    }
    let source = defaults::clock_source();
    let expected =
        defaults::clock_inserted_rows(&source, &[(24, Some(0.0_f64.to_bits())), (25, None)]);
    cases.push((
        "PARAMETERS [Added] LONG; INSERT INTO Items (Id,Score) \
         SELECT [Added],Score FROM Items UNION ALL SELECT [Added]+1,NULL FROM Items",
        "24",
        source,
        expected,
        2,
    ));
    let source = defaults::clock_source();
    cases.push((
        "PARAMETERS [Added] LONG; INSERT INTO Items (Id) \
         SELECT [Added] FROM Items WHERE Id<0",
        "24",
        source.clone(),
        source,
        0,
    ));

    for (sql, value, source, expected, inserted) in cases {
        let (_directory, path, original, mut harness) = opened(source);
        for accept in [false, true] {
            run(&mut harness, sql);
            assert!(harness.state().query_parameters.is_some());
            assert!(harness.state().editing.query_action_review.is_none());
            confirm_parameter(&mut harness, "Added", value);
            finish_reads(&mut harness);
            assert!(matches!(
                harness.state().editing.query_action_review.as_ref(),
                Some((_, ReviewedAction::Insert(_)))
            ));
            assert!(!harness.state().editing.has_save());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            assert!(!journal_path(&path).exists());
            assert_eq!(
                harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
                "99"
            );
            harness
                .get_by_role_and_label(Role::Button, if accept { "Yes" } else { "No" })
                .click_accesskit();
            harness.run_steps(2);
            if !accept {
                assert!(harness.state().editing.query_action_review.is_none());
                assert!(!harness.state().editing.has_save());
                assert_eq!(std::fs::read(&path).unwrap(), original);
                continue;
            }
            let report = finish_save(&mut harness);
            assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.matched_rows.is_none() && receipt.changed_rows == Some(inserted)));
            harness
                .get_by_role_and_label(Role::Button, "Close")
                .click_accesskit();
            harness.run_steps(1);
            finish_reads(&mut harness);
        }
        assert!(
            std::fs::read(&path).unwrap() == expected,
            "complete independent INSERT image differs"
        );
        assert!(!journal_path(&path).exists());
        assert_eq!(harness.state().state.sql, sql);
        assert!(harness.state().state.query_data.is_none());
        run(&mut harness, "SELECT Count(*) AS Remaining FROM Items");
        assert_eq!(
            harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
            (1 + inserted).to_string()
        );
    }
}

mod joined;
mod joined_delete;

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

mod read_source;

mod retained_batch;
