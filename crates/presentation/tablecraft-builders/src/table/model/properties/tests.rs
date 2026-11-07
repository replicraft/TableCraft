use super::{DraftDefaultMode, DraftFieldType, TableDraft, TableDraftError};

fn rejects_unchanged(
    draft: &mut TableDraft,
    expected: TableDraftError,
    change: impl FnOnce(&mut TableDraft) -> Result<(), TableDraftError>,
) {
    let snapshot = draft.snapshot("Items").unwrap();
    let modified = draft.modified;
    let next_field = draft.next_field;
    let retained = draft.retained_bytes();

    assert_eq!(change(draft), Err(expected));
    assert!(draft.snapshot("Items").unwrap() == snapshot);
    assert_eq!(draft.modified, modified);
    assert_eq!(draft.next_field, next_field);
    assert_eq!(draft.retained_bytes(), retained);
}

#[test]
fn new_fields_use_access_ui_policies_without_applying_them_to_autonumber() {
    let mut draft = TableDraft::initial();
    for kind in [
        DraftFieldType::ShortText,
        DraftFieldType::LongInteger,
        DraftFieldType::LargeInteger,
        DraftFieldType::YesNo,
        DraftFieldType::Currency,
        DraftFieldType::DateTime,
    ] {
        draft.add_field(kind).unwrap();
    }
    let snapshot = draft.snapshot("Items").unwrap();

    for (field, captured) in draft.fields().iter().zip(snapshot.fields()) {
        let required = (!field.is_primary()).then_some(false);
        let allow = (field.kind() == DraftFieldType::ShortText).then_some(true);
        assert_eq!(field.required(), required);
        assert_eq!(captured.required(), required);
        assert_eq!(field.allow_zero_length(), allow);
        assert_eq!(captured.allow_zero_length(), allow);
        assert_eq!(field.default_mode(), &DraftDefaultMode::TypeDefault);
        assert_eq!(captured.default_mode(), &DraftDefaultMode::TypeDefault);
        let expected = (field.kind() == DraftFieldType::ShortText).then_some(255);
        assert_eq!(field.short_text_units(), expected);
        assert_eq!(captured.short_text_units(), expected);
    }
}

#[test]
fn applicable_properties_and_exact_raw_default_survive_type_switches_and_snapshots() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft.set_field_text_units(field, 17).unwrap();
    draft.set_field_required(field, Some(true)).unwrap();
    draft
        .set_field_allow_zero_length(field, Some(false))
        .unwrap();
    draft
        .set_field_default(field, DraftDefaultMode::Literal("  '界'  ".into()))
        .unwrap();
    let text_snapshot = draft.snapshot("Items").unwrap();

    draft
        .set_field_type(field, DraftFieldType::Currency)
        .unwrap();
    assert_eq!(draft.fields()[1].short_text_units(), None);
    assert_eq!(draft.fields()[1].allow_zero_length(), None);
    assert_eq!(draft.fields()[1].required(), Some(true));
    assert_eq!(
        draft.fields()[1].default_mode(),
        &DraftDefaultMode::Literal("  '界'  ".into())
    );
    let currency_snapshot = draft.snapshot("Items").unwrap();
    assert_eq!(currency_snapshot.fields()[1].short_text_units(), None);
    assert_eq!(currency_snapshot.fields()[1].allow_zero_length(), None);
    assert_eq!(currency_snapshot.fields()[1].required(), Some(true));
    assert_eq!(
        currency_snapshot.fields()[1].default_mode(),
        &DraftDefaultMode::Literal("  '界'  ".into())
    );

    draft
        .set_field_type(field, DraftFieldType::ShortText)
        .unwrap();
    assert_eq!(draft.fields()[1].id(), field);
    assert_eq!(draft.fields()[1].short_text_units(), Some(17));
    assert_eq!(draft.fields()[1].allow_zero_length(), Some(false));
    assert_eq!(text_snapshot.fields()[1].short_text_units(), Some(17));
    assert_eq!(text_snapshot.fields()[1].allow_zero_length(), Some(false));
    assert_eq!(currency_snapshot.fields()[1].short_text_units(), None);
}

#[test]
fn explicit_false_null_absent_and_raw_literals_remain_distinct_from_omission() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    for required in [Some(true), Some(false), None] {
        let revision = draft.revision();
        draft.set_field_required(field, required).unwrap();
        assert_eq!(draft.revision(), revision + 1);
        assert_eq!(draft.fields()[1].required(), required);
    }
    for allow in [Some(false), Some(true), None] {
        let revision = draft.revision();
        draft.set_field_allow_zero_length(field, allow).unwrap();
        assert_eq!(draft.revision(), revision + 1);
        assert_eq!(draft.fields()[1].allow_zero_length(), allow);
    }
    for mode in [
        DraftDefaultMode::Absent,
        DraftDefaultMode::Null,
        DraftDefaultMode::Literal("NULL".into()),
        DraftDefaultMode::Literal(String::new()),
        DraftDefaultMode::Literal("\0not a parsed default\n".into()),
        DraftDefaultMode::TypeDefault,
    ] {
        let revision = draft.revision();
        draft.set_field_default(field, mode.clone()).unwrap();
        let snapshot = draft.snapshot("Items").unwrap();
        assert_eq!(draft.revision(), revision + 1);
        assert_eq!(draft.fields()[1].default_mode(), &mode);
        assert_eq!(snapshot.fields()[1].default_mode(), &mode);
    }
}

#[test]
fn text_capacity_bounds_and_inapplicable_policies_refuse_without_partial_mutation() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    for units in [1, 255] {
        draft.set_field_text_units(field, units).unwrap();
        assert_eq!(draft.fields()[1].short_text_units(), Some(units));
    }
    for units in [0, 256, u16::MAX] {
        rejects_unchanged(&mut draft, TableDraftError::InvalidProperty, |draft| {
            draft.set_field_text_units(field, units)
        });
    }
    draft
        .set_field_allow_zero_length(field, Some(true))
        .unwrap();
    draft
        .set_field_type(field, DraftFieldType::LongInteger)
        .unwrap();
    rejects_unchanged(&mut draft, TableDraftError::InvalidProperty, |draft| {
        draft.set_field_text_units(field, 1)
    });
    rejects_unchanged(&mut draft, TableDraftError::InvalidProperty, |draft| {
        draft.set_field_allow_zero_length(field, None)
    });
    draft
        .set_field_type(field, DraftFieldType::ShortText)
        .unwrap();
    assert_eq!(draft.fields()[1].short_text_units(), Some(255));
    assert_eq!(draft.fields()[1].allow_zero_length(), Some(true));
}

#[test]
fn every_property_setter_refuses_protected_and_missing_fields_even_for_noops() {
    let mut draft = TableDraft::initial();
    let protected = draft.fields()[0].id();
    let missing = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft.remove_field(missing).unwrap();

    for (field, error) in [
        (protected, TableDraftError::ProtectedField),
        (missing, TableDraftError::MissingField),
    ] {
        rejects_unchanged(&mut draft, error, |draft| {
            draft.set_field_text_units(field, 255)
        });
        rejects_unchanged(&mut draft, error, |draft| {
            draft.set_field_required(field, None)
        });
        rejects_unchanged(&mut draft, error, |draft| {
            draft.set_field_allow_zero_length(field, None)
        });
        rejects_unchanged(&mut draft, error, |draft| {
            draft.set_field_default(field, DraftDefaultMode::TypeDefault)
        });
    }
}

#[test]
fn default_bound_counts_utf8_bytes_and_does_not_retain_excess_caller_capacity() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let maximum = "界".repeat(1365) + "x";
    assert_eq!(maximum.len(), 4096);
    draft
        .set_field_default(field, DraftDefaultMode::Literal(maximum.clone()))
        .unwrap();
    rejects_unchanged(&mut draft, TableDraftError::Limit, |draft| {
        draft.set_field_default(field, DraftDefaultMode::Literal(maximum + "界"))
    });

    let mut overallocated = String::with_capacity(65536);
    overallocated.push_str(" 7 ");
    draft
        .set_field_default(field, DraftDefaultMode::Literal(overallocated))
        .unwrap();
    let DraftDefaultMode::Literal(value) = draft.fields()[1].default_mode() else {
        panic!("Expected exact raw literal");
    };
    assert_eq!(value, " 7 ");
    assert!(value.capacity() < 65536);
}

#[test]
fn revision_exhaustion_refuses_changes_and_accepts_exact_noops() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    draft
        .set_field_default(field, DraftDefaultMode::Literal(" 7 ".into()))
        .unwrap();
    draft.set_field_required(field, None).unwrap();
    draft.set_field_allow_zero_length(field, None).unwrap();
    draft.revision = u64::MAX;

    rejects_unchanged(&mut draft, TableDraftError::Limit, |draft| {
        draft.set_field_text_units(field, 1)
    });
    rejects_unchanged(&mut draft, TableDraftError::Limit, |draft| {
        draft.set_field_required(field, Some(false))
    });
    rejects_unchanged(&mut draft, TableDraftError::Limit, |draft| {
        draft.set_field_allow_zero_length(field, Some(false))
    });
    rejects_unchanged(&mut draft, TableDraftError::Limit, |draft| {
        draft.set_field_default(field, DraftDefaultMode::Absent)
    });

    let original = draft.snapshot("Items").unwrap();
    draft.set_field_text_units(field, 255).unwrap();
    draft.set_field_required(field, None).unwrap();
    draft.set_field_allow_zero_length(field, None).unwrap();
    draft
        .set_field_default(field, DraftDefaultMode::Literal(" 7 ".into()))
        .unwrap();
    assert!(draft.snapshot("Items").unwrap() == original);
}

#[test]
fn retained_byte_accounting_includes_exact_owned_default_buffers() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let empty_model = draft.retained_bytes();
    let empty_snapshot = draft.snapshot("Items").unwrap().retained_bytes();
    draft
        .set_field_default(field, DraftDefaultMode::Literal(" 1234 ".into()))
        .unwrap();
    let DraftDefaultMode::Literal(value) = draft.fields()[1].default_mode() else {
        panic!("Expected owned default input");
    };
    assert_eq!(draft.retained_bytes(), empty_model + value.capacity());
    let snapshot = draft.snapshot("Items").unwrap();
    let DraftDefaultMode::Literal(copied) = snapshot.fields()[1].default_mode() else {
        panic!("Expected captured default input");
    };
    assert_eq!(
        snapshot.retained_bytes(),
        empty_snapshot + copied.capacity()
    );

    draft
        .set_field_default(field, DraftDefaultMode::Absent)
        .unwrap();
    assert_eq!(draft.retained_bytes(), empty_model);
    assert_eq!(
        snapshot.fields()[1].default_mode(),
        &DraftDefaultMode::Literal(" 1234 ".into())
    );
}
