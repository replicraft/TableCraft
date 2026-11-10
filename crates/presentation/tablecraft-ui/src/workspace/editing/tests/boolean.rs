//! A checkbox proposal remains inert until fresh matching runtime admission.
use super::{draft, state};
use crate::workspace::editing::{BooleanToggle, EditDialog, EditInput, InputHint};
use crate::{Cell, CellKind, DisplayControl, DocumentView, EditSeed};

fn boolean(value: bool) -> Cell {
    let mut cell = Cell::scalar(if value { "-1" } else { "0" });
    cell.kind = CellKind::Scalar {
        is_null: false,
        stored_calculation: false,
        boolean: Some(value),
        display_control: DisplayControl::Absent,
    };
    cell
}

#[test]
fn checked_boolean_seed_uses_typed_original_and_never_reviews_or_changes_displayed_data() {
    for expected in [false, true] {
        let (mut state, target) = state();
        state.data.as_mut().unwrap().rows[0][0] = boolean(expected);
        assert!(state.begin_seeded_edit(
            target.clone(),
            EditSeed::Boolean(BooleanToggle::opposite(expected)),
            1,
        ));
        let mut reply = draft(target);
        reply.original = boolean(expected);
        // Runtime BeginEdit returns the typed stored value, not a second
        // display-control inspection. The gesture already captured that policy.
        if let CellKind::Scalar {
            display_control, ..
        } = &mut reply.original.kind
        {
            *display_control = DisplayControl::Uninspected;
        }
        reply.initial = EditInput {
            text: expected.to_string(),
            is_null: false,
        };
        reply.input_hint = InputHint::Boolean;
        reply.nullable = false;
        state.set_edit_draft(1, reply);
        let Some(EditDialog::Draft {
            input,
            review,
            pending,
            ..
        }) = &state.edit
        else {
            panic!("matching fresh Boolean draft must receive the proposal");
        };
        assert_eq!(input.text, (!expected).to_string());
        assert!(!input.is_null);
        assert!(review.is_none());
        assert!(pending.is_none());
        assert_eq!(
            state.data.as_ref().unwrap().rows[0][0].toggle_value(),
            Some(expected)
        );
    }
}

#[test]
fn fresh_stale_wrong_type_control_and_nullable_boolean_drafts_cannot_receive_a_toggle() {
    for failure in 0..6 {
        let (mut state, target) = state();
        state.data.as_mut().unwrap().rows[0][0] = boolean(false);
        assert!(state.begin_seeded_edit(
            target.clone(),
            EditSeed::Boolean(BooleanToggle::opposite(false)),
            1
        ));
        let mut reply = draft(target.clone());
        reply.original = boolean(false);
        reply.initial = EditInput {
            text: "false".into(),
            is_null: false,
        };
        reply.input_hint = InputHint::Boolean;
        reply.nullable = false;
        match failure {
            0 => reply.original = boolean(true),
            1 => reply.input_hint = InputHint::Int32,
            2 => reply.nullable = true,
            3 => reply.initial.is_null = true,
            4 => {
                if let CellKind::Scalar {
                    display_control, ..
                } = &mut reply.original.kind
                {
                    *display_control = DisplayControl::Native(109);
                }
            }
            5 => {
                if let CellKind::Scalar {
                    stored_calculation, ..
                } = &mut reply.original.kind
                {
                    *stored_calculation = true;
                }
            }
            _ => unreachable!(),
        }
        state.set_edit_draft(1, reply);
        assert!(
            matches!(
                &state.edit,
                Some(EditDialog::Checking { error: Some(_), .. })
            ),
            "invalid Boolean draft case {failure}"
        );
        // A second callback cannot revive a consumed, rejected proposal.
        state.set_edit_draft(1, draft(target));
        assert!(matches!(
            &state.edit,
            Some(EditDialog::Checking { error: Some(_), .. })
        ));
    }
}

#[test]
fn toggle_cancellation_late_reply_and_replaced_document_never_seed_an_edit() {
    for replaced in [false, true] {
        let (mut state, target) = state();
        state.data.as_mut().unwrap().rows[0][0] = boolean(false);
        assert!(state.begin_seeded_edit(
            target.clone(),
            EditSeed::Boolean(BooleanToggle::opposite(false)),
            1
        ));
        state.set_edit_draft(9, draft(target.clone()));
        assert!(matches!(state.edit, Some(EditDialog::Checking { .. })));
        if replaced {
            state.database.as_mut().unwrap().document_revision = 8;
        } else {
            assert!(state.close_edit());
        }
        state.set_edit_draft(1, draft(target));
        assert!(!matches!(state.edit, Some(EditDialog::Draft { .. })));
    }
}

#[test]
fn query_calculated_null_textbox_and_uninspected_cells_never_offer_a_toggle() {
    for blocked in 0..6 {
        let (mut state, target) = state();
        let mut cell = boolean(false);
        if let CellKind::Scalar {
            stored_calculation,
            is_null,
            display_control,
            ..
        } = &mut cell.kind
        {
            match blocked {
                0 => state.select_view(DocumentView::Query),
                1 => *stored_calculation = true,
                2 => *is_null = true,
                3 => *display_control = DisplayControl::Native(109),
                4 => *display_control = DisplayControl::Uninspected,
                5 => *display_control = DisplayControl::Unsupported,
                _ => unreachable!(),
            }
        }
        state.data.as_mut().unwrap().rows[0][0] = cell;
        assert!(
            !state.begin_seeded_edit(target, EditSeed::Boolean(BooleanToggle::opposite(false)), 1),
            "blocked Boolean source {blocked}"
        );
        assert!(!state.edit_dialog_open());
    }
}
