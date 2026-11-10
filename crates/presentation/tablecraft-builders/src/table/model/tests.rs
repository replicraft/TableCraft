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

#[test]
fn native_255_field_bound_counts_the_generated_key_and_retains_256th_refusal() {
    let mut draft = TableDraft::initial();
    for index in 1..255 {
        draft.add_field(DraftFieldType::Number).unwrap();
        assert_eq!(
            draft.fields().last().unwrap().name(),
            format!("Field{index}")
        );
    }
    let before = draft.snapshot("Table1").unwrap();
    assert_eq!(before.fields().len(), 255);
    assert_eq!(before.fields()[0].name(), "ID");
    assert_eq!(
        draft.add_field(DraftFieldType::Number),
        Err(TableDraftError::Limit)
    );
    assert!(draft.snapshot("Table1").unwrap() == before);
}
