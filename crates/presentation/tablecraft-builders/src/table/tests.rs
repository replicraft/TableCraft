use super::*;

#[test]
fn initial_autonumber_is_semantic_pristine_and_protected() {
    let mut draft = TableDraft::initial();
    assert!(!draft.is_modified());
    assert_eq!(draft.revision(), 0);
    let id = draft.fields()[0].id();
    assert_eq!(
        draft.fields()[0].kind(),
        DraftFieldType::AutoNumberIncrement
    );
    assert!(draft.fields()[0].is_primary());
    assert_eq!(
        draft.rename_field(id, "Other"),
        Err(TableDraftError::ProtectedField)
    );
    assert!(!draft.is_modified());
}

#[test]
fn field_identity_survives_rename_and_failed_mutations_are_atomic() {
    let mut draft = TableDraft::initial();
    let first = draft.add_field(DraftFieldType::ShortText).unwrap();
    let second = draft.add_field(DraftFieldType::LongInteger).unwrap();
    assert_ne!(first, second);
    draft.rename_field(first, "Customer").unwrap();
    assert_eq!(draft.fields()[1].id(), first);
    assert_eq!(draft.revision(), 3);
    for name in ["", "ID", "customer", "bad.name", "bad\nname", " name"] {
        assert_eq!(
            draft.rename_field(second, name),
            Err(TableDraftError::InvalidName)
        );
        assert_eq!(draft.revision(), 3);
        assert_eq!(draft.fields()[2].name(), "Field2");
    }
    draft.rename_field(second, "Count").unwrap();
    assert_eq!(draft.revision(), 4);
}

#[test]
fn authoring_ceiling_refuses_without_changing_the_last_admitted_revision() {
    let mut draft = TableDraft::initial();
    for _ in 1..16 {
        draft.add_field(DraftFieldType::ShortText).unwrap();
    }
    assert_eq!(draft.fields().len(), 16);
    assert_eq!(
        draft.add_field(DraftFieldType::LongInteger),
        Err(TableDraftError::Limit)
    );
    assert_eq!(draft.revision(), 15);
}
