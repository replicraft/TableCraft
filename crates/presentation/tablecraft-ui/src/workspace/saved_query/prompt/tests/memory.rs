//! Actual prompt acquisition owns a separately admitted retained-memory lifetime.
use super::{Action, Role, State, harness};
use crate::workspace::{
    objects::MAX_RETAINED_BYTES,
    query::parameters::{ParameterInput, ParameterSpec},
};
use egui_kittest::kittest::Queryable;

const SMALL_HEADROOM: usize = 64;
const EDIT_HEADROOM: usize = 8 * 1024;
const REFUSED_EDIT_BYTES: usize = 16 * 1024;

fn pad_existing_result_to(state: &mut State, bytes: usize) {
    let current = state.object_bytes().unwrap();
    assert!(current < bytes);
    let cell = &mut state.saved_query.data.as_mut().unwrap().rows[0][0].text;
    let mut padded = String::with_capacity(cell.capacity() + bytes - current);
    padded.push_str(cell);
    *cell = padded;
    assert_eq!(state.object_bytes(), Some(bytes));
}

fn copied_prompt_heap(state: &State) -> usize {
    let declarations = state.saved_query.parameters.as_ref().unwrap();
    let inputs = &state.saved_query.inputs;
    let declaration_strings = declarations
        .iter()
        .map(|spec| spec.name.len() + spec.kind.len() + spec.label.len())
        .sum::<usize>();
    let input_strings = inputs
        .iter()
        .map(|input| input.name.len() + input.kind.len() + input.text.len())
        .sum::<usize>();
    state.saved_query.target.as_ref().unwrap().name.len()
        + declarations.len() * size_of::<ParameterSpec>()
        + declaration_strings
        + 2 * (inputs.len() * size_of::<ParameterInput>() + input_strings)
}

#[test]
fn prompt_heap_is_charged_while_open_and_cancel_releases_it_without_publishing_inputs() {
    let mut harness = harness();
    harness.run();
    let baseline = harness.state().0.object_bytes().unwrap();
    let minimum_copy_bytes = copied_prompt_heap(&harness.state().0);
    let original = harness.state().0.saved_query.inputs.clone();
    let target = harness.state().0.saved_query.target.clone();
    let rows = harness
        .state()
        .0
        .saved_query
        .data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run();
    assert!(harness.state().0.saved_query_prompt_open());
    assert!(harness.state().0.object_bytes().unwrap() >= baseline + minimum_copy_bytes);
    let prompt_name = original[0].name.clone();
    harness
        .get_by_role_and_label(Role::TextInput, &prompt_name)
        .focus();
    harness.run();
    let before_edit = harness.state().0.object_bytes().unwrap();
    let staged = "pending literal ".repeat(32);
    harness.event(egui::Event::Text(staged.clone()));
    harness.run();
    assert!(harness.state().0.object_bytes().unwrap() >= before_edit + staged.len());
    assert_eq!(harness.state().0.saved_query.inputs, original);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run();
    assert!(!harness.state().0.saved_query_prompt_open());
    assert_eq!(harness.state().0.object_bytes(), Some(baseline));
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(harness.state().0.saved_query.target, target);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn refused_prompt_preserves_result_storage_input_ownership_target_and_queued_actions() {
    let mut harness = harness();
    harness.run();
    pad_existing_result_to(
        &mut harness.state_mut().0,
        MAX_RETAINED_BYTES - SMALL_HEADROOM,
    );
    let original = harness.state().0.saved_query.inputs.clone();
    let input_vector = harness.state().0.saved_query.inputs.as_ptr();
    let input_name = harness.state().0.saved_query.inputs[0].name.as_ptr();
    let target = harness.state().0.saved_query.target.clone();
    let rows = harness
        .state()
        .0
        .saved_query
        .data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let text = harness.state().0.saved_query.data.as_ref().unwrap().rows[0][0]
        .text
        .clone();
    let sql = harness.state().0.sql.clone();
    harness.state_mut().1.push(Action::ChooseFile);
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run();
    assert!(!harness.state().0.saved_query_prompt_open());
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(harness.state().0.saved_query.inputs.as_ptr(), input_vector);
    assert_eq!(
        harness.state().0.saved_query.inputs[0].name.as_ptr(),
        input_name
    );
    assert_eq!(harness.state().0.saved_query.target, target);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
    assert_eq!(
        harness.state().0.saved_query.data.as_ref().unwrap().rows[0][0].text,
        text
    );
    assert_eq!(harness.state().0.sql, sql);
    assert_eq!(harness.state().1.len(), 1);
    assert!(matches!(&harness.state().1[0], Action::ChooseFile));
    assert_eq!(
        harness.state().0.error.as_deref(),
        Some(
            harness
                .state()
                .0
                .localizer
                .text("objects-memory-limit")
                .as_str()
        )
    );
}

#[test]
fn near_quota_prompt_refuses_an_otherwise_bounded_edit_and_cancel_recovers_headroom() {
    let mut harness = harness();
    harness.run();
    pad_existing_result_to(
        &mut harness.state_mut().0,
        MAX_RETAINED_BYTES - EDIT_HEADROOM,
    );
    let baseline = harness.state().0.object_bytes().unwrap();
    let original = harness.state().0.saved_query.inputs.clone();
    let rows = harness
        .state()
        .0
        .saved_query
        .data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run();
    assert!(harness.state().0.saved_query_prompt_open());
    assert!(harness.state().0.object_bytes().unwrap() <= MAX_RETAINED_BYTES);
    let name = original[0].name.clone();
    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run();
    let before_edit = harness.state().0.object_bytes().unwrap();
    let staged = harness
        .state()
        .0
        .saved_query
        .prompt
        .as_ref()
        .unwrap()
        .body
        .draft[0]
        .text
        .clone();
    harness.event(egui::Event::Text("x".repeat(REFUSED_EDIT_BYTES)));
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        staged
    );
    assert_eq!(harness.state().0.object_bytes(), Some(before_edit));
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run();
    assert_eq!(harness.state().0.object_bytes(), Some(baseline));
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
    assert!(
        harness
            .state_mut()
            .0
            .admit_object_bytes(Some(EDIT_HEADROOM / 2))
    );
    assert!(harness.state().1.is_empty());
}
