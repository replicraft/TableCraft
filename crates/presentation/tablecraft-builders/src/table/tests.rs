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

#[test]
fn save_snapshot_is_owned_and_keeps_exact_generation_and_declared_names() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft.rename_field(field, "Customer").unwrap();
    let snapshot = draft.snapshot("Contacts").unwrap();
    draft.rename_field(field, "Supplier").unwrap();
    assert_eq!(snapshot.table_name(), "Contacts");
    assert_eq!(snapshot.revision(), 2);
    assert_eq!(
        snapshot.fields()[0].kind(),
        DraftFieldType::AutoNumberIncrement
    );
    assert!(snapshot.fields()[0].is_primary());
    assert_eq!(snapshot.fields()[1].id(), field);
    assert_eq!(snapshot.fields()[1].name(), "Customer");
    assert_eq!(snapshot.fields()[1].kind(), DraftFieldType::ShortText);
}

#[test]
fn save_names_bound_utf16_and_refuse_invalid_whole_input() {
    let draft = TableDraft::initial();
    assert!(draft.snapshot(&"界".repeat(64)).is_ok());
    assert!(draft.snapshot(&"😀".repeat(32)).is_ok());
    for name in [
        "".into(),
        "bad.name".into(),
        "name ".into(),
        "x".repeat(65),
        "😀".repeat(33),
        "界".repeat(1_000_000),
    ] {
        assert!(matches!(
            draft.snapshot(&name),
            Err(TableDraftError::InvalidName)
        ));
    }
    assert_eq!(draft.revision(), 0);
    assert!(!draft.is_modified());
}

#[test]
fn scalar_gallery_fields_retain_their_exact_kinds_in_the_saved_snapshot() {
    let kinds = [
        DraftFieldType::LongInteger,
        DraftFieldType::LargeInteger,
        DraftFieldType::YesNo,
        DraftFieldType::Currency,
        DraftFieldType::DateTime,
    ];
    let mut draft = TableDraft::initial();
    for kind in kinds {
        draft.add_field(kind).unwrap();
    }
    let snapshot = draft.snapshot("TypedFields").unwrap();
    assert_eq!(
        snapshot.fields()[0].kind(),
        DraftFieldType::AutoNumberIncrement
    );
    assert!(snapshot.fields()[0].is_primary());
    for (field, kind) in snapshot.fields()[1..].iter().zip(kinds) {
        assert_eq!(field.kind(), kind);
        assert!(!field.is_primary());
    }
}
