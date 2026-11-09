//! Selection is inert UI preference; these cases do not supply native save evidence.
use crate::workspace::field_rename::test_support::accepted;
use crate::{DocumentView, Field, Schema, State};

fn schema(number: usize, name: &str) -> Schema {
    let field = |number, name: &str| Field {
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
    };
    Schema {
        table: "Table1".into(),
        declared_rows: 0,
        fields: vec![field(0, "ID"), field(number, name)],
    }
}

fn published() -> State {
    let (mut state, request) = accepted(true);
    state.view = DocumentView::Design;
    let identity = state
        .prepare_field_rename_save(&request, 81, 90)
        .unwrap()
        .bind(100);
    // The same source-free factory supplies matching catalog metadata. Runtime
    // witness authority is a documented caller prerequisite of this UI API.
    let mut database = accepted(true).0.database.unwrap();
    database.document_revision = 8;
    assert!(state.resolve_field_rename_after_reopen(&request, identity, database));
    let before_clear = state.object_bytes().unwrap();
    assert!(state.clear_field_rename_save(&request, identity));
    assert!(
        state.object_bytes().unwrap() <= before_clear,
        "move admitted strings without growing retained payload"
    );
    assert!(state.field_rename.selection.is_some());
    state
}

#[test]
fn fresh_schema_resolves_native_number_seventeen_to_position_one_once() {
    let mut state = published();
    state.set_schema(schema(17, "Total"));
    assert_eq!(state.view, DocumentView::Design);
    assert_eq!(state.selected_field, 1);
    assert!(state.field_rename.selection.is_none());
    assert!(state.error.is_none());
    // Ordinary later schema publication retains its established initial row.
    state.set_schema(schema(17, "Total"));
    assert_eq!(state.selected_field, 0);
}

#[test]
fn same_owner_wrong_name_or_duplicate_native_number_is_an_explicit_restore_failure() {
    for duplicate in [false, true] {
        let mut state = published();
        let mut observed = schema(17, if duplicate { "Total" } else { "Other" });
        if duplicate {
            observed.fields[0].number = 17;
        }
        state.set_schema(observed);
        assert_eq!(state.selected_field, 0);
        assert!(state.error.is_some());
        assert!(state.field_rename.selection.is_none());
    }
}

#[test]
fn later_document_revision_discards_old_selection_without_cross_document_error() {
    let mut state = published();
    state.database.as_mut().unwrap().document_revision = 9;
    state.set_schema(schema(17, "Total"));
    assert_eq!(state.selected_field, 0);
    assert!(state.field_rename.selection.is_none());
    assert!(state.error.is_none());
}
