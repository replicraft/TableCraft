//! Connected scratch prompts route real runtime witnesses without host SQL policy.
use super::*;
use crate::workspace::test_support::{app_harness, finish_reads};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tablecraft_testkit::fixtures::access::{canonical_blank_creation, mutation_fixture};
use tablecraft_ui::{ObjectKind, ObjectTarget, ParameterInput};

fn opened() -> (
    tempfile::TempDir,
    PathBuf,
    Vec<u8>,
    Harness<'static, Desktop>,
) {
    opened_with_source(mutation_fixture::database())
}

fn opened_with_source(
    original: Vec<u8>,
) -> (
    tempfile::TempDir,
    PathBuf,
    Vec<u8>,
    Harness<'static, Desktop>,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("parameters.mdb");
    std::fs::write(&path, &original).unwrap();
    let desktop = Desktop::new(egui::Context::default(), Some(path.clone()));
    let mut harness = app_harness(desktop);
    finish_reads(&mut harness);
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
    (directory, path, original, harness)
}

fn run(harness: &mut Harness<'_, Desktop>, sql: &str) {
    assert!(harness.state_mut().state.replace_query_sql(sql.to_owned()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

fn enter(harness: &mut Harness<'_, Desktop>, prompt: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, prompt)
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text(text.to_owned()));
    harness.run_steps(2);
}

fn confirm(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    // Advancing the stage happens after the current modal has been painted.
    harness.run_steps(2);
}

#[test]
fn normal_run_inspects_then_executes_zero_parameters_and_typed_values_with_explicit_null() {
    let sample =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb");
    let sources = [
        ("Jet4", mutation_fixture::database()),
        ("Ace2007", std::fs::read(sample).unwrap()),
        ("Ace2016", canonical_blank_creation::source()),
    ];
    for (profile, original) in sources {
        let (_directory, path, original, mut harness) = opened_with_source(original);
        assert!(
            harness
                .state()
                .state
                .database
                .as_ref()
                .unwrap()
                .format
                .contains(profile),
            "opened metadata must retain {profile}"
        );
        run(&mut harness, "SELECT 99 AS Previous");
        assert_eq!(
            harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
            "99"
        );
        assert!(harness.state().query_parameters.is_none());

        run(
            &mut harness,
            "PARAMETERS [Text] TEXT(3), [Amount] DECIMAL(5,2), [Nullable] LONG; SELECT [Text], [Amount], [Nullable]",
        );
        assert!(harness.state().query_parameters.is_some());
        assert_eq!(
            harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
            "99"
        );
        enter(&mut harness, "Text", "é🦀");
        confirm(&mut harness);
        enter(&mut harness, "Amount", "12.34");
        confirm(&mut harness);
        harness
            .get_by_role_and_label(Role::CheckBox, "NULL")
            .click_accesskit();
        harness.run_steps(1);
        confirm(&mut harness);
        finish_reads(&mut harness);

        let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
        assert_eq!(rows[0][0].text, "é🦀");
        assert_eq!(rows[0][1].text, "12.34");
        assert!(matches!(
            rows[0][2].kind,
            tablecraft_ui::CellKind::Scalar { is_null: true, .. }
        ));
        assert!(!harness.state().state.query_results_previous());
        assert!(harness.state().query_parameters.is_none());
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}

#[test]
fn cancel_releases_runtime_target_without_evaluating_or_replacing_previous_results() {
    let (_directory, path, original, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    run(&mut harness, "PARAMETERS [Never] LONG; SELECT 1 / 0");
    let target = Arc::downgrade(&harness.state().query_parameters.as_ref().unwrap().target);
    let generation = harness.state().generation;
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    enter(&mut harness, "Never", "7");
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(harness.state().generation, generation);
    assert!(harness.state().worker.poll().is_none());
    assert!(harness.state().query_parameters.is_none());
    assert!(target.upgrade().is_none());
    assert!(harness.state().state.error.is_none());
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
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn typed_constraint_failure_keeps_last_complete_result_and_a_fresh_run_can_correct_it() {
    let (_directory, path, original, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    let sql = "PARAMETERS [Text] TEXT(3); SELECT [Text]";
    run(&mut harness, sql);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    enter(&mut harness, "Text", "four");
    confirm(&mut harness);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(
            Instant::now() < deadline,
            "parameter execution did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
    assert_eq!(
        harness.state().state.error.as_deref(),
        Some(
            tablecraft_runtime::Error::ParameterInput
                .to_string()
                .as_str()
        )
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
        previous
    );
    assert!(harness.state().query_parameters.is_none());

    run(&mut harness, sql);
    enter(&mut harness, "Text", "new");
    confirm(&mut harness);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "new"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn sql_aba_tab_switch_and_document_reopen_release_inspections_and_refuse_old_actions() {
    for transition in ["sql ABA", "tab", "document"] {
        let (_directory, path, original, mut harness) = opened();
        let sql_tab = harness.state().state.active_object().unwrap();
        let sql = "PARAMETERS [Value] LONG; SELECT [Value]";
        run(&mut harness, sql);
        let inspected = harness.state().query_parameters.as_ref().unwrap();
        let id = inspected.inspection_id;
        let target = Arc::downgrade(&inspected.target);
        match transition {
            "sql ABA" => {
                assert!(
                    harness
                        .state_mut()
                        .state
                        .replace_query_sql("SELECT 1".to_owned())
                );
                assert!(harness.state_mut().state.replace_query_sql(sql.to_owned()));
                harness.run_steps(2);
            }
            "tab" => {
                harness
                    .get_by_role_and_label(Role::Button, "Cancel")
                    .click_accesskit();
                harness.run_steps(2);
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
                        name: "Items".to_owned(),
                    },
                });
                finish_reads(&mut harness);
                harness.state_mut().activate_object(sql_tab);
                harness.run_steps(2);
            }
            "document" => {
                harness
                    .get_by_role_and_label(Role::Button, "Cancel")
                    .click_accesskit();
                harness.run_steps(2);
                harness.state_mut().open();
                finish_reads(&mut harness);
            }
            _ => unreachable!(),
        }
        assert!(harness.state().query_parameters.is_none(), "{transition}");
        assert!(target.upgrade().is_none(), "{transition}");
        let generation = harness.state().generation;
        harness.state_mut().run_parameterized_query(
            id,
            vec![ParameterInput {
                name: "Value".to_owned(),
                kind: "int32".to_owned(),
                text: "7".to_owned(),
                is_null: false,
            }],
        );
        assert_eq!(harness.state().generation, generation, "{transition}");
        assert!(harness.state().worker.poll().is_none(), "{transition}");
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}

#[test]
fn duplicate_and_late_inspection_callbacks_cannot_dismiss_an_accepted_parameter_prompt() {
    let (_directory, path, original, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    run(&mut harness, "PARAMETERS [Value] LONG; SELECT [Value]");
    let inspected = harness.state().query_parameters.as_ref().unwrap();
    let id = inspected.inspection_id;
    let target = Arc::clone(&inspected.target);
    let generation = harness.state().generation;
    let rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();

    for callback_id in [id, id - 1] {
        harness
            .state_mut()
            .publish_query_parameters(callback_id, Arc::clone(&target), Vec::new());
        assert_eq!(harness.state().generation, generation);
        let retained = harness.state().query_parameters.as_ref().unwrap();
        assert_eq!(retained.inspection_id, id);
        assert!(Arc::ptr_eq(&retained.target, &target));
        assert!(
            harness
                .state()
                .state
                .query_parameter_inspection_current(id, target.sql())
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
        assert!(harness.state().worker.poll().is_none());
    }

    harness.run_steps(2);
    enter(&mut harness, "Value", "7");
    confirm(&mut harness);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "7"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn stale_run_intent_cannot_replace_an_accepted_prompt_or_its_raw_input() {
    let (_directory, path, original, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    run(&mut harness, "PARAMETERS [Text] TEXT(64); SELECT [Text]");
    let raw = "  Résumé 日本語 🦀  ";
    enter(&mut harness, "Text", raw);
    let inspected = harness.state().query_parameters.as_ref().unwrap();
    let id = inspected.inspection_id;
    let target = Arc::clone(&inspected.target);
    let generation = harness.state().generation;
    let rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();

    harness.state_mut().inspect_query_parameters();
    assert_eq!(harness.state().generation, generation);
    let retained = harness.state().query_parameters.as_ref().unwrap();
    assert_eq!(retained.inspection_id, id);
    assert!(Arc::ptr_eq(&retained.target, &target));
    assert!(
        harness
            .state()
            .state
            .query_parameter_inspection_current(id, target.sql())
    );
    assert!(harness.state().worker.poll().is_none());
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
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Text")
            .value()
            .as_deref(),
        Some(raw)
    );

    confirm(&mut harness);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        raw
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn explicit_read_limits_are_captured_before_inspection_and_retry_requires_an_explicit_run() {
    let (_directory, path, original, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    let mut limits = harness.state().state.query_run_limits().unwrap();
    limits.query.rows = 1;
    harness.state_mut().configure_query_run_limits(limits);
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql("SELECT Id FROM Items".into())
    );
    harness.state_mut().inspect_query_parameters();
    let captured = match harness.state().pending {
        PendingOperation::QueryInspection { options, .. } => options,
        _ => panic!("actual read inspection owns captured limits"),
    };
    assert_eq!(captured.query().rows(), 1);
    let mut larger = limits;
    larger.query.rows = 100;
    harness.state_mut().configure_query_run_limits(larger);
    assert_eq!(harness.state().query_run_options, captured);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(Instant::now() < deadline);
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(harness.state().state.error.is_some());
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    harness.state_mut().configure_query_run_limits(larger);
    assert_eq!(harness.state().query_run_options.query().rows(), 100);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99",
        "configuring limits must never run SQL implicitly"
    );
    run(&mut harness, "SELECT Id FROM Items");
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
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn active_parameter_prompt_rejects_injected_query_limit_configuration() {
    let (_directory, path, original, mut harness) = opened();
    run(
        &mut harness,
        "PARAMETERS [Requested] LONG; SELECT [Requested]",
    );
    let captured = harness.state().query_run_options;
    let mut changed = harness.state().state.query_run_limits().unwrap();
    changed.query.rows += 1;
    harness.state_mut().configure_query_run_limits(changed);
    assert_eq!(harness.state().query_run_options, captured);
    enter(&mut harness, "Requested", "42");
    confirm(&mut harness);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "42"
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn native_source_failures_preserve_sql_and_previous_result_until_an_explicit_corrected_run() {
    let (_directory, path, original, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    let defaults = harness.state().state.query_run_limits().unwrap();
    let sql = "SELECT Id FROM Items";
    for snapshot in [true, false] {
        let mut limits = defaults;
        if snapshot {
            limits.snapshot_rows = 2;
        } else {
            limits.native.rows = 4;
        }
        harness.state_mut().configure_query_run_limits(limits);
        assert!(harness.state_mut().state.replace_query_sql(sql.into()));
        harness.state_mut().inspect_query_parameters();
        let deadline = Instant::now() + Duration::from_secs(10);
        while harness.state().state.busy {
            assert!(Instant::now() < deadline);
            harness.run_steps(1);
            std::thread::yield_now();
        }
        let error = harness.state().state.error.as_deref().unwrap();
        if snapshot {
            assert!(error.contains("source"), "{error}");
            assert!(error.contains("structur"), "{error}");
        } else {
            assert!(error.contains("Native pool"), "{error}");
            assert!(error.contains("Rows resource limit exceeded"), "{error}");
        }
        assert_eq!(harness.state().state.sql, sql);
        assert_eq!(
            harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
            "99"
        );
    }
    harness.state_mut().configure_query_run_limits(defaults);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99",
        "configuration never silently retries a failed source read"
    );
    run(&mut harness, sql);
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
    assert_eq!(std::fs::read(path).unwrap(), original);
}
