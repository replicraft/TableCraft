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
    let second = draft.add_field(DraftFieldType::Number).unwrap();
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
        draft.add_field(DraftFieldType::Number),
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
        DraftFieldType::Number,
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

#[test]
fn removing_protected_or_missing_fields_leaves_a_pristine_draft_unchanged() {
    let mut draft = TableDraft::initial();
    let initial = draft.snapshot("Items").unwrap();
    let protected = draft.fields()[0].id();
    let missing = TableDraft::initial()
        .add_field(DraftFieldType::ShortText)
        .unwrap();

    for (id, expected) in [
        (protected, TableDraftError::ProtectedField),
        (missing, TableDraftError::MissingField),
    ] {
        assert_eq!(draft.remove_field(id), Err(expected));
        assert!(draft.snapshot("Items").unwrap() == initial);
        assert!(!draft.is_modified());
        assert_eq!(draft.revision(), 0);
    }
}

#[test]
fn removing_a_middle_field_preserves_surviving_declarations_and_old_snapshots() {
    let mut draft = TableDraft::initial();
    let first = draft.add_field(DraftFieldType::ShortText).unwrap();
    let removed = draft.add_field(DraftFieldType::Currency).unwrap();
    let last = draft.add_field(DraftFieldType::YesNo).unwrap();
    draft.rename_field(first, "Customer").unwrap();
    draft.rename_field(last, "Enabled").unwrap();
    let original = draft.snapshot("Items").unwrap();
    let revision = draft.revision();

    draft.remove_field(removed).unwrap();

    assert_eq!(draft.revision(), revision + 1);
    assert!(draft.is_modified());
    assert_eq!(draft.fields().len(), 3);
    for (field, expected) in draft.fields().iter().zip([
        (
            original.fields()[0].id(),
            "ID",
            DraftFieldType::AutoNumberIncrement,
            true,
        ),
        (first, "Customer", DraftFieldType::ShortText, false),
        (last, "Enabled", DraftFieldType::YesNo, false),
    ]) {
        assert_eq!(
            (field.id(), field.name(), field.kind(), field.is_primary()),
            expected
        );
    }
    assert_eq!(original.fields().len(), 4);
    assert_eq!(original.fields()[2].id(), removed);
    assert_eq!(original.revision(), revision);

    let current = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.remove_field(removed),
        Err(TableDraftError::MissingField)
    );
    assert!(draft.snapshot("Items").unwrap() == current);
    assert!(draft.is_modified());
}

#[test]
fn removing_fields_releases_capacity_without_reusing_identifiers_or_resetting_dirty_state() {
    let mut draft = TableDraft::initial();
    let mut issued = Vec::new();
    for _ in 1..16 {
        issued.push(draft.add_field(DraftFieldType::ShortText).unwrap());
    }
    let removed = issued[6];
    draft.remove_field(removed).unwrap();
    let replacement = draft.add_field(DraftFieldType::Currency).unwrap();

    assert!(!issued.contains(&replacement));
    assert_eq!(draft.fields().len(), 16);
    assert_eq!(draft.fields().last().unwrap().id(), replacement);
    assert_eq!(draft.revision(), 17);
    assert_eq!(
        draft.remove_field(removed),
        Err(TableDraftError::MissingField)
    );

    for id in issued.into_iter().filter(|id| *id != removed) {
        draft.remove_field(id).unwrap();
    }
    draft.remove_field(replacement).unwrap();
    assert_eq!(draft.fields().len(), 1);
    assert_eq!(
        draft.fields()[0].kind(),
        DraftFieldType::AutoNumberIncrement
    );
    assert!(draft.is_modified());
    assert_eq!(draft.revision(), 32);
}

#[test]
fn field_type_changes_preserve_identity_name_order_and_owned_snapshot() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let neighbor = draft.add_field(DraftFieldType::DateTime).unwrap();
    draft.rename_field(field, "Amount").unwrap();
    let original = draft.snapshot("Items").unwrap();

    for kind in [
        DraftFieldType::Number,
        DraftFieldType::LargeInteger,
        DraftFieldType::YesNo,
        DraftFieldType::Currency,
        DraftFieldType::DateTime,
        DraftFieldType::ShortText,
    ] {
        let revision = draft.revision();
        draft.set_field_type(field, kind).unwrap();
        assert_eq!(draft.revision(), revision + 1);
        assert!(draft.is_modified());
        assert_eq!(draft.fields().len(), 3);
        assert_eq!(draft.fields()[1].id(), field);
        assert_eq!(draft.fields()[1].name(), "Amount");
        assert_eq!(draft.fields()[1].kind(), kind);
        assert!(!draft.fields()[1].is_primary());
        assert_eq!(draft.fields()[2].id(), neighbor);
        assert_eq!(draft.fields()[2].kind(), DraftFieldType::DateTime);

        let current = draft.snapshot("Items").unwrap();
        draft.set_field_type(field, kind).unwrap();
        assert!(draft.snapshot("Items").unwrap() == current);
    }
    assert_eq!(original.fields()[1].kind(), DraftFieldType::ShortText);
    assert_eq!(original.fields()[1].name(), "Amount");
    assert_eq!(original.revision(), 3);
}

#[test]
fn protected_and_missing_field_type_changes_refuse_atomically() {
    let mut draft = TableDraft::initial();
    let protected = draft.fields()[0].id();
    let pristine = draft.snapshot("Items").unwrap();
    for kind in [
        DraftFieldType::ShortText,
        DraftFieldType::AutoNumberIncrement,
    ] {
        assert_eq!(
            draft.set_field_type(protected, kind),
            Err(TableDraftError::ProtectedField)
        );
        assert!(draft.snapshot("Items").unwrap() == pristine);
        assert!(!draft.is_modified());
    }

    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let current = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.set_field_type(field, DraftFieldType::AutoNumberIncrement),
        Err(TableDraftError::ProtectedField)
    );
    assert!(draft.snapshot("Items").unwrap() == current);
    draft.remove_field(field).unwrap();
    let current = draft.snapshot("Items").unwrap();
    assert_eq!(
        draft.set_field_type(field, DraftFieldType::Currency),
        Err(TableDraftError::MissingField)
    );
    assert!(draft.snapshot("Items").unwrap() == current);
}
