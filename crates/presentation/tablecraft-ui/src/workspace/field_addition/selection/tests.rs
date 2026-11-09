//! Inert selection preferences; runtime witnesses remain a host prerequisite.
use crate::workspace::{DocumentView, test_support};
use crate::{Column, EditSaveOutcome, Field, Schema, State};
use tablecraft_builders::table::FieldAdditionDeclaration;

use super::super::{FieldAdditionRequest, FieldAdditionSaveIdentity};

fn field(number: usize, name: &str) -> Field {
    Field {
        name: name.into(),
        number,
        native_type: 4,
        storage_bytes: 4,
        fixed: true,
        flags: 0,
        extended_flags: 0,
        scalar_type: Some("Int32".into()),
        display_type: None,
        field_size: None,
        reader_nullable: None,
        decimal_parameters: None,
        complex_type_id: None,
        properties: Vec::new(),
    }
}

fn schema() -> Schema {
    Schema {
        table: "Table1".into(),
        declared_rows: 0,
        // The added field is deliberately not last and its native number is not
        // its row index. Selection must resolve actual accepted metadata.
        fields: vec![field(0, "ID"), field(17, "Added"), field(4, "Existing")],
    }
}

fn base() -> State {
    test_support::saved_table(
        "Design addition",
        vec![Column {
            name: "ID".into(),
            caption: None,
            native_number: Some(0),
        }],
    )
}

fn saving() -> (State, FieldAdditionRequest, FieldAdditionSaveIdentity) {
    let mut state = base();
    state.field_addition_available = true;
    let tab = state.active_object().unwrap();
    assert!(state.begin_field_addition_inspection(&tab, 10).is_ok());
    assert!(state.set_field_addition_availability(&tab, 10, true, false));
    state.prepare_schema_view();
    state.set_schema(Schema {
        table: "Table1".into(),
        declared_rows: 0,
        fields: vec![field(0, "ID")],
    });
    assert!(state.view == DocumentView::Design);
    assert!(state.begin_field_addition_name(
        &tab,
        "Added".into(),
        FieldAdditionDeclaration::NumberLongZero,
    ));
    let request = state.field_addition_review_request().unwrap();
    assert!(state.begin_field_addition_review(&request, 20));
    assert!(state.accept_field_addition_review(&request, 20, 30));
    assert!(state.bind_field_addition_save(&request, 20, 30, 40));
    let identity = FieldAdditionSaveIdentity {
        request_id: 20,
        review_id: 30,
        save_id: 40,
    };
    assert!(state.begin_field_addition_save_status(identity));
    assert!(
        state.set_field_addition_save_outcome(
            identity,
            EditSaveOutcome::Saved("known AFTER".into()),
        )
    );
    assert!(state.begin_field_addition_reopen(identity));
    (state, request, identity)
}

fn published() -> State {
    let (mut state, request, identity) = saving();
    let mut fresh = base().database.unwrap();
    fresh.document_revision = 8;
    // Source-free UI tests represent the host's successful full-witness handoff;
    // this catalog alone supplies neither native save nor replacement authority.
    assert!(state.resolve_field_addition_after_reopen(20, &request, 30, 40, fresh));
    assert!(state.view == DocumentView::Design);
    let before = state.object_bytes().unwrap();
    assert!(state.clear_field_addition_save(&request, identity));
    assert!(state.object_bytes().unwrap() <= before);
    assert!(state.field_addition.selection.is_some());
    state
}

#[test]
fn checked_addition_preserves_design_and_selects_unique_actual_native_field_once() {
    let mut state = published();
    state.set_schema(schema());
    assert!(state.view == DocumentView::Design);
    assert_eq!(state.selected_field, 1);
    assert_eq!(state.schema.as_ref().unwrap().fields[1].number, 17);
    assert!(state.field_addition.selection.is_none());
    assert!(state.error.is_none());

    state.set_schema(schema());
    assert_eq!(state.selected_field, 0);
}

#[test]
fn missing_name_and_duplicate_name_or_native_number_refuse_presentation_restore() {
    for malformed in [
        vec![field(0, "ID"), field(17, "Other")],
        vec![field(0, "ID"), field(17, "Added"), field(4, "Added")],
        vec![field(17, "ID"), field(17, "Added")],
    ] {
        let mut state = published();
        state.set_schema(Schema {
            table: "Table1".into(),
            declared_rows: 0,
            fields: malformed,
        });
        assert_eq!(state.selected_field, 0);
        assert!(state.field_addition.selection.is_none());
        assert!(state.error.is_some());
    }
}

#[test]
fn original_ack_or_wrong_save_identity_does_not_create_selection_or_drop_the_owner() {
    let (mut state, request, identity) = saving();
    let mut wrong = identity;
    wrong.save_id += 1;
    assert!(!state.clear_field_addition_save(&request, wrong));
    assert!(state.matches_field_addition_save(&request, identity));
    assert!(state.field_addition.selection.is_none());

    // An original/no-write acknowledgement keeps the current document revision.
    assert!(state.clear_field_addition_save(&request, identity));
    assert!(state.field_addition.selection.is_none());
    state.set_schema(schema());
    assert_eq!(state.selected_field, 0);
}

#[test]
fn later_revision_or_another_active_view_cannot_receive_old_selection() {
    let mut state = published();
    state.database.as_mut().unwrap().document_revision = 9;
    state.set_schema(schema());
    assert_eq!(state.selected_field, 0);
    assert!(state.field_addition.selection.is_none());
    assert!(state.error.is_none());

    let mut state = published();
    state.view = DocumentView::Table;
    state.set_schema(schema());
    assert_eq!(state.selected_field, 0);
    assert!(state.field_addition.selection.is_none());
    assert!(state.error.is_none());
}
