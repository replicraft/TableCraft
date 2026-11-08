use super::{DraftFieldType, TableDraft, TableDraftError};

#[test]
fn type_change_revision_exhaustion_refuses_without_changing_any_draft_state() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft.revision = u64::MAX;
    let original = draft.snapshot("Items").unwrap();
    let next_field = draft.next_field;
    let modified = draft.modified;

    assert_eq!(
        draft.set_field_type(field, DraftFieldType::Currency),
        Err(TableDraftError::Limit)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    assert_eq!(draft.next_field, next_field);
    assert_eq!(draft.modified, modified);

    assert_eq!(
        draft.set_field_type(field, DraftFieldType::ShortText),
        Ok(())
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    assert_eq!(draft.next_field, next_field);
    assert_eq!(draft.modified, modified);
}

mod named;
