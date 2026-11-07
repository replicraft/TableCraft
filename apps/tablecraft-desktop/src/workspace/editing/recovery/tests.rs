//! Actual restart prompts and retained save ownership, without native windows.
use super::*;
use crate::workspace::test_support::{app_harness, finish_reads};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tablecraft_runtime::{SaveCompletion, SaveKnownOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::support::access::restart::{RestartImages, stage, stage_first_table};

struct RestartCase {
    harness: Harness<'static, Desktop>,
    path: PathBuf,
    journal: PathBuf,
    images: RestartImages,
    directory: tempfile::TempDir,
}

fn blocked(committed: bool) -> RestartCase {
    blocked_from("restart.mdb", committed, stage)
}

fn blocked_from(
    filename: &str,
    committed: bool,
    stage: fn(&Path, &Path, bool) -> RestartImages,
) -> RestartCase {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(filename);
    let journal = journal_path(&path);
    let images = stage(&path, &journal, committed);
    let before_open = snapshot(&journal);
    let desktop = Desktop::new(egui::Context::default(), Some(path.clone()));
    let mut harness = app_harness(desktop);
    finish_reads(&mut harness);

    assert!(harness.state().state.recovery_prompt_open());
    assert!(harness.state().state.database.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(fs::read(&path).unwrap(), images.after);
    assert_eq!(snapshot(&journal), before_open);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Recover")
            .is_some()
    );
    RestartCase {
        harness,
        path,
        journal,
        images,
        directory,
    }
}

fn snapshot(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(base: &Path, path: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(base, &entry.unwrap().path(), output);
            }
        } else {
            output.push((
                path.strip_prefix(base).unwrap().to_path_buf(),
                fs::read(path).unwrap(),
            ));
        }
    }
    let mut output = Vec::new();
    visit(path, path, &mut output);
    output.sort();
    output
}

fn click(harness: &mut Harness<'_, Desktop>, label: &str) {
    {
        let mut buttons = harness
            .get_all_by_role_and_label(Role::Button, label)
            .filter(|node| !node.accesskit_node().is_disabled());
        let button = buttons.next().expect("one enabled recovery action");
        assert!(buttons.next().is_none(), "recovery action is ambiguous");
        button.click_accesskit();
    }
    harness.run_steps(1);
}

fn terminal(harness: &mut Harness<'_, Desktop>) -> SaveReport {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(report) = harness
            .state()
            .recovery
            .as_ref()
            .and_then(|owner| owner.operation.as_ref())
            .and_then(|operation| operation.report)
        {
            harness.run_steps(2);
            return report;
        }
        assert!(Instant::now() < deadline, "restart recovery did not finish");
        harness.run_steps(1);
        std::thread::yield_now();
    }
}

fn begin(harness: &mut Harness<'_, Desktop>) -> SaveReport {
    click(harness, "Recover");
    terminal(harness)
}

fn assert_close_veto(harness: &mut Harness<'_, Desktop>) {
    let context = harness.ctx.clone();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = context.run_ui(input, |ui| {
        harness.state_mut().guard_save_close(ui.ctx());
    });
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    output.textures_delta.clear();
}

#[test]
fn blocked_open_and_prompt_cancel_leave_actual_database_and_journal_untouched() {
    let mut case = blocked(false);
    let journal_before = snapshot(&case.journal);
    let attempt = case.harness.state().recovery.as_ref().unwrap().attempt;
    case.harness
        .state_mut()
        .begin_reserved_recovery(attempt + 1);
    case.harness
        .state_mut()
        .close_reserved_recovery(attempt + 1);
    case.harness.run_steps(2);
    assert_eq!(
        case.harness.state().state.recovery_attempt_id(),
        Some(attempt)
    );
    assert!(case.harness.state().editing.worker.poll().is_none());

    click(&mut case.harness, "Cancel");
    assert!(case.harness.state().recovery.is_none());
    assert!(!case.harness.state().state.recovery_prompt_open());
    assert!(case.harness.state().state.database.is_none());
    assert_eq!(fs::read(&case.path).unwrap(), case.images.after);
    assert_eq!(snapshot(&case.journal), journal_before);
}

#[test]
fn pending_and_committed_restart_recover_captured_path_then_require_acknowledgment_to_open() {
    for committed in [false, true] {
        let mut case = blocked(committed);
        let other = case.directory.path().join("later-selection.mdb");
        fs::write(&other, b"not the selected recovery target").unwrap();
        case.harness.state_mut().state.path = other.to_string_lossy().into_owned();
        case.harness.state_mut().file_selected(Some(other.clone()));
        assert_eq!(
            case.harness.state().recovery.as_ref().unwrap().path,
            case.path
        );

        let report = begin(&mut case.harness);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("actual restart recovery failed: {report:?}");
        };
        assert_eq!(
            receipt.known,
            if committed {
                SaveKnownOutcome::Committed
            } else {
                SaveKnownOutcome::RolledBack
            }
        );
        let expected = if committed {
            &case.images.after
        } else {
            &case.images.before
        };
        assert_eq!(&fs::read(&case.path).unwrap(), expected);
        assert_eq!(
            fs::read(&other).unwrap(),
            b"not the selected recovery target"
        );
        assert!(!case.journal.exists());
        assert!(case.harness.state().state.database.is_none());
        assert!(case.harness.state().state.data.is_none());
        assert!(case.harness.state().editing.has_save());
        assert!(
            case.harness
                .state()
                .state
                .can_acknowledge_recovery(report.id)
        );

        click(&mut case.harness, "Close");
        finish_reads(&mut case.harness);
        assert!(case.harness.state().recovery.is_none());
        assert!(!case.harness.state().editing.has_save());
        assert_eq!(
            case.harness.state().state.data.as_ref().unwrap().table,
            "Items"
        );
        assert!(
            !case
                .harness
                .state()
                .state
                .data
                .as_ref()
                .unwrap()
                .rows
                .is_empty()
        );
        assert_eq!(&fs::read(&case.path).unwrap(), expected);
    }
}

#[test]
fn ace16_first_table_v2_restart_preserves_schema_facts_and_reopens_only_after_acknowledgment() {
    for committed in [false, true] {
        let mut case = blocked_from("schema-restart.accdb", committed, stage_first_table);
        let report = begin(&mut case.harness);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("actual ACE16 first-table restart failed: {report:?}");
        };
        assert_eq!(receipt.completion, SaveCompletion::Recovered);
        assert_eq!(
            receipt.known,
            if committed {
                SaveKnownOutcome::Committed
            } else {
                SaveKnownOutcome::RolledBack
            }
        );
        assert_eq!(
            receipt.schema_change,
            committed.then_some(SaveSchemaChange::FirstTableCreated { column_count: 2 })
        );
        assert_eq!(receipt.matched_rows, None);
        assert_eq!(receipt.changed_rows, None);
        let expected = if committed {
            &case.images.after
        } else {
            &case.images.before
        };
        assert_eq!(&fs::read(&case.path).unwrap(), expected);
        assert!(!case.journal.exists());
        assert!(case.harness.state().state.database.is_none());
        assert!(case.harness.state().state.data.is_none());
        assert!(case.harness.state().editing.has_save());
        assert!(
            case.harness
                .state()
                .state
                .can_acknowledge_recovery(report.id)
        );

        click(&mut case.harness, "Close");
        finish_reads(&mut case.harness);
        let database = case.harness.state().state.database.as_ref().unwrap();
        assert_eq!(
            database.tables.iter().any(|table| table.name == "Items"),
            committed
        );
        if committed {
            let data = case.harness.state().state.data.as_ref().unwrap();
            assert_eq!(data.table, "Items");
            assert!(data.rows.is_empty());
            assert_eq!(
                data.columns
                    .iter()
                    .map(|column| column.name.as_str())
                    .collect::<Vec<_>>(),
                ["Id", "Name"]
            );
        } else {
            assert!(case.harness.state().state.data.is_none());
        }
        assert!(case.harness.state().recovery.is_none());
        assert!(!case.harness.state().editing.has_save());
        assert_eq!(&fs::read(&case.path).unwrap(), expected);
    }
}

#[test]
fn malformed_and_foreign_journals_refuse_and_explicit_release_preserves_the_files() {
    for foreign in [false, true] {
        let mut case = blocked(false);
        fs::rename(
            &case.journal,
            case.directory.path().join("original-journal"),
        )
        .unwrap();
        let foreign_path = case.directory.path().join("foreign.mdb");
        let foreign_image = if foreign {
            Some(stage(&foreign_path, &case.journal, true).after)
        } else {
            fs::write(&case.journal, b"malformed reserved journal").unwrap();
            None
        };
        let journal_before = snapshot(&case.journal);
        let report = begin(&mut case.harness);
        let SaveOutcome::Failed(failure) = report.outcome else {
            panic!("unqualified recovery must refuse: {report:?}");
        };
        assert!(failure.can_release);
        assert!(!failure.can_recover);
        assert!(
            !case
                .harness
                .state()
                .state
                .can_acknowledge_recovery(report.id)
        );
        assert_eq!(fs::read(&case.path).unwrap(), case.images.after);
        assert_eq!(snapshot(&case.journal), journal_before);

        click(&mut case.harness, "Release resources");
        let released = terminal(&mut case.harness);
        assert!(matches!(released.outcome, SaveOutcome::Released { .. }));
        click(&mut case.harness, "Close");
        assert!(case.harness.state().recovery.is_none());
        assert!(case.harness.state().state.database.is_none());
        assert_eq!(fs::read(&case.path).unwrap(), case.images.after);
        assert_eq!(snapshot(&case.journal), journal_before);
        if let Some(expected) = foreign_image {
            assert_eq!(fs::read(&foreign_path).unwrap(), expected);
        }
    }
}

#[test]
fn stale_operation_actions_cannot_replace_or_acknowledge_actual_recovery() {
    let mut case = blocked(true);
    let attempt = case.harness.state().recovery.as_ref().unwrap().attempt;
    case.harness.state_mut().begin_reserved_recovery(attempt);
    let operation = case
        .harness
        .state()
        .recovery
        .as_ref()
        .unwrap()
        .operation
        .as_ref()
        .unwrap()
        .id;
    case.harness
        .state_mut()
        .cancel_reserved_recovery(operation + 1);
    case.harness
        .state_mut()
        .resolve_reserved_recovery(operation + 1, true);
    case.harness
        .state_mut()
        .acknowledge_reserved_recovery(operation + 1);
    let report = terminal(&mut case.harness);
    assert_eq!(report.id, operation);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    case.harness
        .state_mut()
        .acknowledge_reserved_recovery(operation + 1);
    assert!(case.harness.state().state.database.is_none());
    assert_eq!(case.harness.state().editing.worker.poll(), Some(report));
    click(&mut case.harness, "Close");
    finish_reads(&mut case.harness);
    assert_eq!(fs::read(&case.path).unwrap(), case.images.after);
}

#[test]
fn recovery_accepted_in_the_close_frame_retains_its_owner_before_window_exit() {
    let mut case = blocked(false);
    case.harness
        .get_by_role_and_label(Role::Button, "Recover")
        .click_accesskit();
    case.harness
        .input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    case.harness.run_steps(1);
    let vetoed = case.harness.output().viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&egui::ViewportCommand::CancelClose);
    let retained = case.harness.state().editing.has_save();
    case.harness
        .input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .clear();

    let report = terminal(&mut case.harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    click(&mut case.harness, "Close");
    finish_reads(&mut case.harness);
    assert_eq!(fs::read(&case.path).unwrap(), case.images.before);
    assert!(retained, "the new operation must remain owned");
    assert!(
        vetoed,
        "an operation accepted during UI dispatch must veto exit"
    );
}

#[test]
fn running_and_unacknowledged_recovery_veto_window_close_without_dropping_owner() {
    let mut case = blocked(false);
    let attempt = case.harness.state().recovery.as_ref().unwrap().attempt;
    case.harness.state_mut().begin_reserved_recovery(attempt);
    assert!(case.harness.state().state.recovery_pending());
    assert_close_veto(&mut case.harness);
    assert!(case.harness.state().recovery.is_some());
    let mut report = terminal(&mut case.harness);
    assert_close_veto(&mut case.harness);
    assert_eq!(case.harness.state().editing.worker.poll(), Some(report));

    // Cancellation requests stopping; the actual terminal result decides
    // whether resources need an explicit release before acknowledgment.
    if let SaveOutcome::Failed(failure) = report.outcome
        && failure.can_release
    {
        click(&mut case.harness, "Release resources");
        report = terminal(&mut case.harness);
    }
    assert!(
        case.harness
            .state()
            .state
            .can_acknowledge_recovery(report.id)
    );
    click(&mut case.harness, "Close");
    if case.harness.state().state.busy {
        finish_reads(&mut case.harness);
    }
    assert!(case.harness.state().recovery.is_none());
}

#[test]
fn failed_post_recovery_reopen_preserves_the_acknowledged_completion_fact() {
    let mut case = blocked(true);
    let report = begin(&mut case.harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    assert_eq!(fs::read(&case.path).unwrap(), case.images.after);
    fs::write(&case.path, b"changed after acknowledged recovery").unwrap();
    click(&mut case.harness, "Close");
    let deadline = Instant::now() + Duration::from_secs(10);
    while case.harness.state().state.busy {
        assert!(
            Instant::now() < deadline,
            "post-recovery reopen did not finish"
        );
        case.harness.run_steps(1);
        std::thread::yield_now();
    }
    case.harness.run_steps(2);
    assert!(case.harness.state().recovery.is_none());
    assert!(case.harness.state().recovery_reopen.is_none());
    assert!(case.harness.state().state.database.is_none());
    let message = case.harness.state().state.error.as_deref().unwrap();
    assert!(
        message.contains("committed"),
        "known completion lost: {message}"
    );
    assert!(message.contains("Last completed operation:"));
    assert_eq!(
        fs::read(&case.path).unwrap(),
        b"changed after acknowledged recovery"
    );
}
