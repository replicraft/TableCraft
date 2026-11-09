//! Actual naming widgets compose runtime review, save and guarded closed-object reopen.
use super::*;
use crate::workspace::test_support::{self, finish_reads, finish_save};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::{SaveKnownOutcome, SaveSchemaChange};
use tablecraft_testkit::fixtures::access::{
    canonical_catalog_names::rename_pair, canonical_indexed_rename,
    canonical_ordinary_indexed::Case,
};

#[test]
fn closed_navigation_rename_saves_once_and_reselects_without_reopening_table() {
    for indexed in [None, Some(Case::CounterPrimary), Some(Case::RetainedTree)] {
        let (before, after) = match indexed {
            Some(case) => {
                let source = canonical_indexed_rename::pair(6, case, "Items", "雪 Customers");
                (source.before, source.after)
            }
            None => {
                let source = rename_pair(6, "Items", "雪 Customers", true);
                (source.before, source.after)
            }
        };
        let (_directory, path, _before, mut harness) = test_support::opened(before);
        let report = save_rename(&mut harness);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::TableRenamed)
            && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
        assert!(harness.state().editing.has_save());
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert_eq!(std::fs::read(path).unwrap(), after);
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.table_rename_open());
        assert!(harness.state().state.object_tabs().is_empty());
        assert!(harness.state().state.data.is_none());
        assert!(matches!(harness.state().state.selected_navigation_object(),
        Some(ObjectTarget { kind: ObjectKind::Table { name }, .. }) if name == "雪 Customers"));
    }
}

#[test]
fn unchanged_name_is_reviewed_but_does_not_start_a_save_or_open_table() {
    let source = rename_pair(5, "Items", "Items", false);
    let (_directory, path, before, mut harness) = test_support::opened(source.before);
    let tab = harness.state().state.active_object().unwrap();
    harness.state_mut().close_object(tab);
    harness.run_steps(2);
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert!(harness.state_mut().state.begin_table_rename(ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::Table {
            name: "Items".into()
        },
    }));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Rename")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().table_rename.is_none());
    assert!(!harness.state().state.table_rename_open());
    assert!(harness.state().state.object_tabs().is_empty());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

// Exercise terminal-envelope consumption with genuine saved bytes, accepted
// capability and AFTER witness. These substitutions are not native fault injection.
#[test]
fn released_known_replacement_reopens_but_current_original_takes_precedence() {
    let cases = [
        (Some(SaveKnownOutcome::Committed), false, true),
        (Some(SaveKnownOutcome::ReplacementVerified), false, true),
        (None, true, true),
        (Some(SaveKnownOutcome::RolledBack), true, false),
        (Some(SaveKnownOutcome::OriginalVerified), true, false),
    ];
    for (current_outcome, completed, reopens) in cases {
        let source = rename_pair(6, "Items", "雪 Customers", true);
        let (_directory, path, _before, mut harness) = test_support::opened(source.before);
        let mut report = save_rename(&mut harness);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("real rename save did not complete: {report:?}");
        };
        assert!(
            SaveOutcome::Saved { receipt }
                .table_rename_reopen_facts()
                .unwrap()
                .replacement
        );
        assert_eq!(std::fs::read(&path).unwrap(), source.after);
        report.outcome = SaveOutcome::Released {
            current_outcome,
            last_completed: completed.then_some(receipt),
        };
        harness.state_mut().editing.save.as_mut().unwrap().report = Some(report);
        harness
            .state_mut()
            .state
            .begin_table_rename_resolution(report.id, report.id);
        harness.state_mut().table_rename_save_report(report);
        // Consume the substituted envelope before ordinary polling restores
        // the real worker's stable Saved report. The actual Close path is
        // exercised separately by the connected naming/save test above.
        harness.state_mut().acknowledge_table_rename(report.id);
        if reopens {
            harness.run_steps(1);
            finish_reads(&mut harness);
        } else {
            assert!(!harness.state().state.busy);
            assert!(!matches!(
                harness.state().pending,
                PendingOperation::RenamedTableReopen { .. }
            ));
            harness.run_steps(2);
        }

        assert!(!harness.state().editing.has_save());
        assert!(harness.state().table_rename.is_none());
        assert!(harness.state().state.object_tabs().is_empty());
        assert!(harness.state().state.data.is_none());
        let selected = harness.state().state.selected_navigation_object().unwrap();
        let ObjectKind::Table { name } = &selected.kind else {
            panic!("expected table selection");
        };
        assert_eq!(name, if reopens { "雪 Customers" } else { "Items" });
        assert_eq!(std::fs::read(path).unwrap(), source.after);
    }
}

// A reserved-path sentinel exercises the real read guard, not journal recovery
// or a native finalization fault. The known commit must survive its refusal.
#[test]
fn released_commit_retains_known_result_when_reserved_journal_blocks_reopen() {
    let source = rename_pair(6, "Items", "雪 Customers", true);
    let (_directory, path, _before, mut harness) = test_support::opened(source.before);
    let mut report = save_rename(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if SaveOutcome::Saved { receipt }.table_rename_reopen_facts().is_ok_and(|facts| facts.replacement))
    );
    report.outcome = SaveOutcome::Released {
        current_outcome: Some(SaveKnownOutcome::Committed),
        last_completed: None,
    };
    harness.state_mut().editing.save.as_mut().unwrap().report = Some(report);
    harness
        .state_mut()
        .state
        .begin_table_rename_resolution(report.id, report.id);
    harness.state_mut().table_rename_save_report(report);
    std::fs::write(journal_path(&path), b"reserved read-guard sentinel").unwrap();
    // A normal frame would repoll the real worker's stable Saved report;
    // this consumer-only check acknowledges the substituted envelope directly.
    harness.state_mut().acknowledge_table_rename(report.id);
    harness.run_steps(1);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(
            std::time::Instant::now() < deadline,
            "guarded reopen did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
    let error = harness.state().state.error.as_ref().unwrap();
    let known = harness
        .state()
        .state
        .localizer
        .text("editing-save-known-committed");
    assert!(error.contains(&known));
    assert!(error.contains(&tablecraft_runtime::Error::RetainedJournal.to_string()));
    assert!(harness.state().state.database.is_none());
    assert!(harness.state().state.object_tabs().is_empty());
    assert!(harness.state().table_rename.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(path).unwrap(), source.after);
}

fn save_rename(harness: &mut Harness<'_, Desktop>) -> SaveReport {
    let tab = harness.state().state.active_object().unwrap();
    harness.state_mut().close_object(tab);
    harness.run_steps(2);
    assert!(harness.state().state.object_tabs().is_empty());
    harness.get_by_role_and_label(Role::Button, "Items").focus();
    harness.run_steps(1);
    harness.key_press(Key::F2);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "New name")
        .focus();
    harness
        .get_by_role_and_label(Role::TextInput, "New name")
        .type_text("雪 Customers");
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_save(harness)
}
