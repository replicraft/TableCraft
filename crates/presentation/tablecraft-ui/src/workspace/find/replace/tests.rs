use super::*;
use crate::workspace::find::tests::{page, state};
use crate::workspace::find::{FindMatch, FindSpan};
use crate::{Action, Cell, EditInput, InputHint};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn matched() -> State {
    let mut state = state();
    state.replace_available = true;
    state.find.as_mut().unwrap().matching = FindMatch::Any;
    state.open_replace();
    let request = state.current_find_request().unwrap();
    state.begin_find(1, request.clone());
    assert!(state.set_find_result(
        1,
        request,
        Some(FindPosition {
            parent_ordinal: 100,
            column: "Name".into(),
            span: Some(FindSpan {
                start_utf16: 0,
                end_utf16: 3
            }),
        }),
        Some(page(100))
    ));
    state
}

fn draft(initial: &str) -> EditDraft {
    EditDraft {
        id: 11,
        target: EditTarget {
            document_revision: 7,
            table: "People".into(),
            column: "Name".into(),
            parent_ordinal: 100,
        },
        original: Cell::scalar("Ada Ada"),
        initial: EditInput {
            text: initial.into(),
            is_null: false,
        },
        input_hint: InputHint::Text,
        nullable: true,
    }
}

#[test]
fn replacement_requires_match_receipt_and_enters_review_without_splicing_or_null_inference() {
    let mut state = matched();
    let request = state.current_replace_find_request().unwrap();
    assert!(request.find.start.as_ref().unwrap().span.is_none());
    assert!(
        state
            .current_find_request()
            .unwrap()
            .start
            .unwrap()
            .span
            .is_some()
    );
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 0, 0);
    assert!(
        state.current_replace_find_request().is_none(),
        "an arbitrary selected cell is not a match"
    );
    state
        .table_selection
        .select(state.data.as_ref().unwrap(), 0, 1);
    assert!(state.can_replace_find(&request));
    assert!(state.begin_replace_find(2, request.clone()));
    let mut malformed = draft(" Ada");
    malformed.target.column = "ID".into();
    assert!(!state.set_replace_find_draft(2, malformed));
    assert!(state.find.as_ref().unwrap().error.is_some());
    assert!(state.edit.is_none());
    assert!(state.begin_replace_find(3, request));
    assert_eq!(state.close_find(), Some(3));
    state.open_replace();
    assert!(!state.set_replace_find_draft(3, draft(" Ada")));
    assert!(state.edit.is_none());

    for (replacement, initial) in [("", " Ada"), ("NULL", "NULL Ada")] {
        let mut state = matched();
        state.find.as_mut().unwrap().replacement.text = replacement.into();
        let request = state.current_replace_find_request().unwrap();
        assert!(state.begin_replace_find(4, request));
        assert!(state.set_replace_find_draft(4, draft(initial)));
        assert!(!state.find_window_open());
        let Some(EditDialog::Draft {
            input,
            pending,
            review,
            ..
        }) = &state.edit
        else {
            panic!("replacement must enter the ordinary draft editor");
        };
        assert_eq!(input.text, initial);
        assert!(!input.is_null);
        assert!(pending.is_none());
        assert!(review.is_none());
        assert!(!state.can_apply_edit(11));
    }
}

#[test]
fn actual_replace_tabs_preserve_inputs_on_oversized_events_and_invalidate_changed_search() {
    let mut state = matched();
    state.find.as_mut().unwrap().replacement.visible = false;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Replace")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Tab, "Replace")
            .accesskit_node()
            .is_selected(),
        Some(true)
    );
    harness
        .get_by_role_and_label(Role::Tab, "Find")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Tab, "Replace")
        .click_accesskit();
    harness.run_steps(2);
    let input = harness.get_by_role_and_label(Role::TextInput, "Replace With:");
    input.focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, "Replace With:")
        .type_text("keep");
    harness.run_steps(2);
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("x".repeat(65_536)));
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.find.as_ref().unwrap().replacement.text,
        "keep"
    );
    assert_eq!(
        harness.state().0.find.as_ref().unwrap().error.as_deref(),
        Some(
            harness
                .state()
                .0
                .localizer
                .text("find-replacement-limit")
                .as_str()
        )
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Replace All")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness.state().1.is_empty(),
        "editing controls must not prepare or apply a replacement"
    );

    harness
        .get_by_role_and_label(Role::TextInput, "Find What:")
        .focus();
    harness.run_steps(1);
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("🦀".repeat(65_536)));
    harness.run_steps(2);
    assert_eq!(harness.state().0.find.as_ref().unwrap().pattern, "Ada");
    assert!(harness.state().0.current_replace_find_request().is_some());
    harness
        .get_by_role_and_label(Role::TextInput, "Find What:")
        .type_text("X");
    harness.run_steps(2);
    assert!(harness.state().0.current_replace_find_request().is_none());
    assert!(
        harness
            .state()
            .0
            .find
            .as_ref()
            .unwrap()
            .found_request
            .is_none()
    );
}
