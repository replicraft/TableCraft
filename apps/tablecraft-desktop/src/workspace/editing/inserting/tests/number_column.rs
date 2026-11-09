//! Actual Number header save followed by an omitted-default GUI record save.
use super::review;
use crate::workspace::test_support::{self, finish_reads, finish_save};
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::{SaveOutcome, journal_path};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;
use tablecraft_ui::CellKind;

#[test]
fn saved_number_then_new_row_omits_score_and_generates_id_through_real_widgets() {
    let before = raw::source(raw::Source::IdOnly);
    let after_column = raw::append(&before, raw::Source::IdOnly, "Score");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    // Only the Number host switch is overridden here. Native source
    // qualification, ordinary insertion availability and every actual
    // review/save/reopen retain their normal host behavior.
    harness.state_mut().state.number_column_available = true;
    harness.state_mut().inspect_number_column_availability();
    finish_reads(&mut harness);
    let original_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Number")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text("Score");
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(
        !harness.state().editing.has_save(),
        "Number must finish its full-image checked reopen before New"
    );
    assert!(!harness.state().state.number_column_open());
    assert_eq!(std::fs::read(&path).unwrap(), after_column);
    assert!(!journal_path(&path).exists());
    let schema_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert_ne!(schema_revision, original_revision);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(
        page.columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["ID", "Score"]
    );
    assert!(page.rows.is_empty());

    // The ordinary Ribbon starts a real source-bound insertion. Neither ID nor
    // Score is supplied by this test; no display string becomes an edit value.
    let new = harness.get_by_role_and_label(Role::Button, "New");
    assert!(!new.accesskit_node().is_disabled());
    new.click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let generated = harness.get_by_role_and_label(Role::Label, "(New)");
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::SetValue)
    );
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus)
    );
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "ID")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::MultilineTextInput, "ID")
            .is_none()
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Score")
            .value()
            .as_deref(),
        Some("")
    );
    // Change the actual mode, then explicitly omit it. Both transitions close
    // the real menu; no literal zero or empty numeric value is submitted.
    for mode in ["Value", "Omitted"] {
        harness
            .get_by_role_and_label(Role::Button, "Score input mode")
            .click_accesskit();
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::RadioButton, mode)
            .click_accesskit();
        harness.run_steps(2);
    }
    review(&mut harness);
    let review_id = harness.state().editing.insert_review.as_ref().unwrap().0;
    assert!(harness.state().state.can_apply_insert(review_id));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        after_column,
        "row review is inert"
    );
    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1)
            && receipt.schema_change.is_none()
            && receipt.query_change.is_none()),
        "{report:?}"
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());
    let reopened = harness.state().state.data.as_ref().unwrap();
    assert_eq!(reopened.table, "Table1");
    assert_ne!(reopened.source_revision, Some(schema_revision));
    assert_eq!(reopened.rows.len(), 1);
    assert_eq!(
        reopened.rows[0][0].text, "1",
        "the native generator supplies ID"
    );
    assert_eq!(
        reopened.rows[0][1].text, "0",
        "omitted Score uses its stored default"
    );
    for cell in &reopened.rows[0] {
        assert!(matches!(cell.kind, CellKind::Scalar { is_null: false, .. }));
    }
}
