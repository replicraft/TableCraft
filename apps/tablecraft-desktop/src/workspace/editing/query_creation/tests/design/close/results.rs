//! Completed saved Design results keep their actual editor as the close owner.
use super::super::run;
use super::*;

#[test]
fn active_saved_design_run_then_close_saves_its_current_model_through_tcqe() {
    let (_directory, path, case, clock, mut harness) = opened();
    let expected = query_expression_edits::replace(&case.before, "101");
    saved(&mut harness, "Saved");
    let original = harness.state().state.active_object().unwrap();
    design(&mut harness);
    edit(&mut harness, "Answer: 101");
    run(&mut harness, "101");
    assert!(harness.state().state.has_modified_query_design());
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "101"
    );
    assert_eq!(std::fs::read(&path).unwrap(), case.before);
    assert_eq!(clock.calls(), 0);

    // Prior rows may be viewed while raw Field text is unfinished, but the
    // author must still be able to return to its current editor and cancel it.
    design(&mut harness);
    edit(&mut harness, "Answer: IIf('");
    assert!(harness.state().state.has_unresolved_query_design_edits());
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Datasheet")
        .click_accesskit();
    harness.run_steps(2);
    design(&mut harness);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Answer: IIf('")
    );
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().state.has_unresolved_query_design_edits());
    assert_eq!(std::fs::read(&path).unwrap(), case.before);

    close(&mut harness);
    assert!(harness.state().state.query_design_close_open());
    assert!(!harness.state().state.saved_query_sql_close_open());
    choose(&mut harness, "saved-query-close-save");
    finish_schema_save(&mut harness);
    assert!(!harness.state().state.query_creation_name_open());
    assert!(!harness.state().editing.has_save());
    assert!(
        !harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == original.id)
    );
    assert!(!harness.state().state.can_activate_object(&original));
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    saved(&mut harness, "Saved");
    run_saved(&mut harness, "101");
    assert_eq!(std::fs::read(path).unwrap(), expected);
    assert_eq!(clock.calls(), 0);
}
