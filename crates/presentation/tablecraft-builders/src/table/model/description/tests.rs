use super::super::{DraftFieldId, DraftFieldType, TableDraft};
use crate::table::{MAX_DRAFT_DESCRIPTION_UNITS, TableDraftError};

#[test]
fn descriptions_preserve_presence_unicode_whitespace_and_protected_id_metadata() {
    let mut draft = TableDraft::initial();
    let id = draft.fields()[0].id();
    assert_eq!(draft.fields()[0].description(), None);
    draft.set_field_description(id, Some("")).unwrap();
    assert_eq!(draft.fields()[0].description(), Some(""));
    let empty = draft.snapshot("Items").unwrap();
    assert_eq!(empty.fields()[0].description(), Some(""));
    let revision = draft.revision();
    draft.set_field_description(id, Some("")).unwrap();
    assert_eq!(draft.revision(), revision);
    draft
        .set_field_description(id, Some("  É漢字 😀\t\n "))
        .unwrap();
    assert_eq!(
        draft.snapshot("Items").unwrap().fields()[0].description(),
        Some("  É漢字 😀\t\n ")
    );
    assert!(draft.fields()[0].is_primary());
    assert_eq!(
        draft.fields()[0].kind(),
        DraftFieldType::AutoNumberIncrement
    );
    draft.set_field_description(id, None).unwrap();
    assert_eq!(draft.fields()[0].description(), None);
    assert!(draft.snapshot("Items").unwrap() != empty);
}

#[test]
fn exact_utf16_boundary_and_refusals_leave_every_draft_fact_unchanged() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let boundary = format!("{}a", "😀".repeat(127));
    assert_eq!(boundary.encode_utf16().count(), MAX_DRAFT_DESCRIPTION_UNITS);
    let before_bytes = draft.retained_bytes();
    draft.set_field_description(field, Some(&boundary)).unwrap();
    assert!(draft.retained_bytes() >= before_bytes + boundary.len());
    let original = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.set_field_description(field, Some(&(boundary.clone() + "a"))),
        Err(TableDraftError::InvalidDescription)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    assert_eq!(
        draft.set_field_description(DraftFieldId(u64::MAX), Some("missing")),
        Err(TableDraftError::MissingField)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    draft
        .set_field_type(field, DraftFieldType::Currency)
        .unwrap();
    assert_eq!(draft.fields()[1].description(), Some(boundary.as_str()));
}

#[test]
fn exhausted_revision_refuses_changes_but_accepts_exact_noop() {
    let mut draft = TableDraft::initial();
    let field = draft.fields()[0].id();
    draft
        .set_field_description(field, Some("Original"))
        .unwrap();
    draft.revision = u64::MAX;
    let original = draft.snapshot("Items").unwrap();
    let modified = draft.modified;
    let next_field = draft.next_field;
    assert_eq!(
        draft.set_field_description(field, Some("Changed")),
        Err(TableDraftError::Limit)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    assert_eq!(draft.modified, modified);
    assert_eq!(draft.next_field, next_field);
    assert_eq!(draft.set_field_description(field, Some("Original")), Ok(()));
    assert!(draft.snapshot("Items").unwrap() == original);
}
