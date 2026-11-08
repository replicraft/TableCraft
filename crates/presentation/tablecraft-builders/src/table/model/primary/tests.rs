use super::super::{DraftDefaultMode, DraftFieldId, DraftFieldType, DraftNumberSize, TableDraft};
use crate::table::{MAX_DRAFT_FIELDS, TableDraftError};

#[test]
fn manual_design_starts_empty_and_key_selection_does_not_protect_or_reorder_fields() {
    let mut draft = TableDraft::design();
    assert!(draft.is_design());
    assert!(draft.fields().is_empty());
    assert!(!draft.is_modified());
    assert_eq!(draft.revision(), 0);
    let name = draft.add_field(DraftFieldType::ShortText).unwrap();
    let key = draft.add_field(DraftFieldType::Number).unwrap();
    draft
        .set_field_description(key, Some("  exact 日本語  "))
        .unwrap();
    draft.set_field_required(key, Some(false)).unwrap();
    draft
        .set_field_default(key, DraftDefaultMode::Literal("12".into()))
        .unwrap();
    draft.set_primary_key(Some(key)).unwrap();
    let snapshot = draft.snapshot("Items").unwrap();
    assert_eq!(snapshot.fields()[0].id(), name);
    assert_eq!(snapshot.fields()[1].id(), key);
    assert!(snapshot.fields()[1].is_primary());
    assert!(!snapshot.fields()[1].is_protected());
    assert_eq!(snapshot.fields()[1].required(), Some(false));
    assert_eq!(snapshot.fields()[1].description(), Some("  exact 日本語  "));
    draft.rename_field(key, "Manual key").unwrap();
    draft
        .set_field_type(key, DraftFieldType::LargeInteger)
        .unwrap();
    assert_eq!(draft.fields()[1].id(), key);
    assert!(draft.fields()[1].is_primary());
    draft.remove_field(key).unwrap();
    assert_eq!(draft.fields()[0].id(), name);
    assert!(draft.fields().iter().all(|field| !field.is_primary()));
    assert!(
        snapshot.fields()[1].is_primary(),
        "captured declarations are immutable"
    );
}

#[test]
fn incompatible_primary_type_size_missing_id_and_overflow_refuse_atomically() {
    let mut draft = TableDraft::design();
    let text = draft.add_field(DraftFieldType::ShortText).unwrap();
    let key = draft.add_field(DraftFieldType::Number).unwrap();
    assert_eq!(
        draft.set_primary_key(Some(text)),
        Err(TableDraftError::InvalidPrimaryKey)
    );
    draft.set_primary_key(Some(key)).unwrap();
    let original = draft.snapshot("Items").unwrap();
    for kind in [
        DraftFieldType::ShortText,
        DraftFieldType::YesNo,
        DraftFieldType::DateTime,
    ] {
        assert_eq!(
            draft.set_field_type(key, kind),
            Err(TableDraftError::InvalidPrimaryKey)
        );
        assert!(draft.snapshot("Items").unwrap() == original);
    }
    for size in [
        DraftNumberSize::Byte,
        DraftNumberSize::Integer,
        DraftNumberSize::Single,
        DraftNumberSize::Double,
        DraftNumberSize::Decimal,
        DraftNumberSize::ReplicationId,
    ] {
        assert_eq!(
            draft.set_field_number_size(key, size),
            Err(TableDraftError::InvalidPrimaryKey)
        );
        assert!(draft.snapshot("Items").unwrap() == original);
    }
    assert_eq!(
        draft.set_primary_key(Some(DraftFieldId(u64::MAX))),
        Err(TableDraftError::MissingField)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    draft.revision = u64::MAX;
    let exhausted = draft.snapshot("Items").unwrap();
    assert_eq!(draft.set_primary_key(None), Err(TableDraftError::Limit));
    assert_eq!(draft.set_primary_key(Some(key)), Ok(()));
    assert!(draft.snapshot("Items").unwrap() == exhausted);
}

#[test]
fn key_can_move_between_supported_fields_and_hidden_number_size_is_not_discarded() {
    let mut draft = TableDraft::design();
    let first = draft.add_field(DraftFieldType::Number).unwrap();
    let second = draft.add_field(DraftFieldType::Currency).unwrap();
    draft.set_primary_key(Some(first)).unwrap();
    draft.set_primary_key(Some(second)).unwrap();
    assert!(!draft.fields()[0].is_primary());
    assert!(draft.fields()[1].is_primary());
    draft
        .set_field_type(second, DraftFieldType::Number)
        .unwrap();
    assert_eq!(
        draft.fields()[1].number_size(),
        Some(DraftNumberSize::LongInteger)
    );
    assert!(draft.fields()[1].is_primary());
    draft
        .set_field_type(second, DraftFieldType::Currency)
        .unwrap();
    draft.set_primary_key(None).unwrap();
    draft
        .set_field_type(second, DraftFieldType::Number)
        .unwrap();
    draft
        .set_field_number_size(second, DraftNumberSize::Byte)
        .unwrap();
    draft
        .set_field_type(second, DraftFieldType::Currency)
        .unwrap();
    draft.set_primary_key(Some(second)).unwrap();
    let original = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.set_field_type(second, DraftFieldType::Number),
        Err(TableDraftError::InvalidPrimaryKey)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
}

#[test]
fn automatic_id_prepends_a_fresh_protected_identity_without_overwriting_collisions() {
    let mut draft = TableDraft::design();
    let first = draft.add_field(DraftFieldType::ShortText).unwrap();
    let second = draft.add_field(DraftFieldType::Number).unwrap();
    draft.rename_field(first, "id").unwrap();
    draft.rename_field(second, "ID1").unwrap();
    draft
        .set_field_description(first, Some("Retained description"))
        .unwrap();
    let new = draft.add_incrementing_primary_key().unwrap();
    assert_ne!(new, first);
    assert_ne!(new, second);
    assert_eq!(draft.fields()[0].name(), "ID2");
    assert!(draft.fields()[0].is_primary());
    assert!(draft.fields()[0].is_protected());
    assert_eq!(draft.fields()[1].id(), first);
    assert_eq!(draft.fields()[2].id(), second);
    assert_eq!(
        draft.fields()[1].description(),
        Some("Retained description")
    );
    assert_eq!(
        draft.rename_field(new, "Other"),
        Err(TableDraftError::ProtectedField)
    );
    assert_eq!(
        draft.remove_field(new),
        Err(TableDraftError::ProtectedField)
    );
    assert_eq!(
        draft.set_primary_key(None),
        Err(TableDraftError::ProtectedField)
    );
    assert!(draft.is_design());

    let initial = TableDraft::initial();
    assert!(!initial.is_design());
    assert!(initial.snapshot("Items").unwrap().fields()[0].is_protected());
}

#[test]
fn automatic_id_capacity_and_identity_overflow_leave_all_user_fields_unchanged() {
    let mut draft = TableDraft::design();
    for _ in 0..MAX_DRAFT_FIELDS {
        draft.add_field(DraftFieldType::Number).unwrap();
    }
    let original = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.add_incrementing_primary_key(),
        Err(TableDraftError::Limit)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    let last = draft.fields().last().unwrap().id();
    draft.remove_field(last).unwrap();
    draft.next_field = u64::MAX;
    let original = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.add_incrementing_primary_key(),
        Err(TableDraftError::Limit)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
    draft.next_field = 100;
    draft.revision = u64::MAX;
    let original = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.add_incrementing_primary_key(),
        Err(TableDraftError::Limit)
    );
    assert!(draft.snapshot("Items").unwrap() == original);
}
