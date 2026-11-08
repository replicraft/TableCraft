//! Actual editor, Save shortcut, retained query modal and explicit Run dispatch.
use super::{open_query, storage_pair};
use crate::workspace::test_support::{finish_reads, finish_save, opened};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::{SaveOutcome, SaveQueryChange};

#[test]
fn editor_save_shortcut_modal_acknowledgement_and_witnessed_run_use_actual_host_dispatch() {
    let [before, after] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before);
    open_query(&mut harness);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    let submitted = " select  43 as [Answer];\n";
    harness.event(Event::Paste(submitted.into()));
    harness.run_steps(1);
    assert_eq!(
        harness.state().state.saved_query_sql().unwrap().1,
        submitted
    );
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Run saved query")
            .accesskit_node()
            .is_disabled()
    );

    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.query_change == Some(SaveQueryChange::ExpressionSaved)));
    assert!(
        harness
            .state()
            .state
            .saved_query_sql_save_status()
            .is_some()
    );
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert!(harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), after);

    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(
        harness
            .state()
            .state
            .saved_query_sql_save_status()
            .is_none()
    );
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.has_modified_saved_query_sql());
    let (original, proposed, _, _) = harness.state().state.saved_query_sql().unwrap();
    assert_eq!(original, submitted);
    assert_eq!(proposed, submitted);
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Run saved query")
            .accesskit_node()
            .is_disabled()
    );

    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.selected_cell_text(), Some("43"));
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1
    );
}
