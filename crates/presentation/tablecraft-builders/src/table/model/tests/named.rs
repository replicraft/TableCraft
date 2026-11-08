use super::super::super::{DraftDefaultMode, DraftNumberSize};
use super::super::DraftFieldId;
use super::{DraftFieldType, TableDraft, TableDraftError};
use crate::table::MAX_DRAFT_FIELDS;

#[test]
fn rejected_named_insertions_preserve_exact_schema_and_next_identity() {
    let mut draft = TableDraft::initial();
    draft
        .add_named_field(DraftFieldType::ShortText, "Name")
        .unwrap();
    let original = draft.snapshot("Items").unwrap();
    let next_field = draft.next_field;
    let modified = draft.modified;
    for name in [
        "",
        " Name",
        "Name ",
        "name",
        "ID",
        "Bad.Field",
        "Bad[Field]",
        "line\nbreak",
    ] {
        assert_eq!(
            draft.add_named_field(DraftFieldType::Currency, name),
            Err(TableDraftError::InvalidName)
        );
        assert!(draft.snapshot("Items").unwrap() == original);
        assert_eq!(draft.next_field, next_field);
        assert_eq!(draft.modified, modified);
    }
    for name in ["a".repeat(65), "😀".repeat(33)] {
        assert_eq!(
            draft.add_named_field(DraftFieldType::ShortText, &name),
            Err(TableDraftError::InvalidName)
        );
        assert!(draft.snapshot("Items").unwrap() == original);
        assert_eq!(draft.next_field, next_field);
    }
    assert_eq!(
        draft.add_named_field(DraftFieldType::AutoNumberIncrement, "OtherId"),
        Err(TableDraftError::ProtectedField)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    assert_eq!(draft.next_field, next_field);
}

#[test]
fn full_and_exhausted_named_insertions_never_leave_an_intermediate_field() {
    let mut full = TableDraft::design();
    for index in 0..MAX_DRAFT_FIELDS {
        full.add_named_field(DraftFieldType::ShortText, &format!("F{index}"))
            .unwrap();
    }
    let original = full.snapshot("Items").unwrap();
    let next_field = full.next_field;
    assert_eq!(
        full.add_named_field(DraftFieldType::Number, "Extra"),
        Err(TableDraftError::Limit)
    );
    assert!(full.snapshot("Items").unwrap() == original);
    assert_eq!(full.next_field, next_field);

    for revision_exhausted in [false, true] {
        let mut draft = TableDraft::design();
        if revision_exhausted {
            draft.revision = u64::MAX;
        } else {
            draft.next_field = u64::MAX;
        }
        let original = draft.snapshot("Items").unwrap();
        let next_field = draft.next_field;
        assert_eq!(
            draft.add_named_field(DraftFieldType::Number, "Score"),
            Err(TableDraftError::Limit)
        );
        assert!(draft.snapshot("Items").unwrap() == original);
        assert_eq!(draft.next_field, next_field);
        assert!(!draft.modified);
    }
}

#[test]
fn named_insertion_uses_one_revision_and_ordinary_defaults_without_inventing_protection() {
    let mut draft = TableDraft::design();
    let id = draft
        .add_named_field(DraftFieldType::Number, "郵便番号")
        .unwrap();
    assert_eq!(id, DraftFieldId(1));
    assert_eq!(draft.revision(), 1);
    assert_eq!(draft.next_field, 2);
    assert!(draft.is_modified());
    let field = &draft.fields()[0];
    assert_eq!(field.name(), "郵便番号");
    assert_eq!(field.kind(), DraftFieldType::Number);
    assert_eq!(field.number_size(), Some(DraftNumberSize::LongInteger));
    assert_eq!(field.required(), Some(false));
    assert_eq!(field.default_mode(), &DraftDefaultMode::TypeDefault);
    assert_eq!(field.description(), None);
    assert!(!field.is_primary() && !field.is_protected());
    draft.rename_field(id, "Score").unwrap();
    draft.remove_field(id).unwrap();
    let next = draft
        .add_named_field(DraftFieldType::ShortText, "Name")
        .unwrap();
    assert_eq!(next, DraftFieldId(2));
    assert_eq!(draft.fields()[0].allow_zero_length(), Some(true));
}

#[test]
fn named_fields_compose_with_existing_generated_name_selection_and_ascii_duplicate_policy() {
    let mut draft = TableDraft::initial();
    let protected = draft.fields()[0].id();
    draft
        .add_named_field(DraftFieldType::ShortText, "Field1")
        .unwrap();
    let generated = draft.add_field(DraftFieldType::ShortText).unwrap();
    assert_eq!(
        draft
            .fields()
            .iter()
            .find(|field| field.id() == generated)
            .unwrap()
            .name(),
        "Field2"
    );
    draft
        .add_named_field(DraftFieldType::ShortText, "É")
        .unwrap();
    draft
        .add_named_field(DraftFieldType::ShortText, "é")
        .unwrap();
    assert_eq!(
        draft.remove_field(protected),
        Err(TableDraftError::ProtectedField)
    );
    assert_eq!(draft.fields()[0].name(), "ID");
    assert!(draft.fields()[0].is_primary() && draft.fields()[0].is_protected());
}
