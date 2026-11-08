use super::*;

#[test]
fn number_defaults_sizes_and_effective_snapshots_preserve_hidden_choices() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::Number).unwrap();
    assert_eq!(
        draft.fields()[1].number_size(),
        Some(DraftNumberSize::LongInteger)
    );
    assert!(draft.fields()[1].decimal_constraints().is_none());

    for size in [
        DraftNumberSize::Byte,
        DraftNumberSize::Integer,
        DraftNumberSize::LongInteger,
        DraftNumberSize::Single,
        DraftNumberSize::Double,
        DraftNumberSize::Decimal,
        DraftNumberSize::ReplicationId,
    ] {
        draft.set_field_number_size(field, size).unwrap();
        let snapshot = draft.snapshot("Numbers").unwrap();
        assert_eq!(snapshot.fields()[1].number_size(), Some(size));
        assert_eq!(
            snapshot.fields()[1].decimal_constraints(),
            (size == DraftNumberSize::Decimal).then(|| DecimalConstraints::new(18, 0).unwrap())
        );
    }

    draft
        .set_field_number_size(field, DraftNumberSize::Decimal)
        .unwrap();
    let constraints = DecimalConstraints::new(12, 5).unwrap();
    draft
        .set_field_decimal_constraints(field, constraints)
        .unwrap();
    draft
        .set_field_type(field, DraftFieldType::ShortText)
        .unwrap();
    let hidden = draft.snapshot("Numbers").unwrap();
    assert!(hidden.fields()[1].number_size().is_none());
    assert!(hidden.fields()[1].decimal_constraints().is_none());
    draft.set_field_type(field, DraftFieldType::Number).unwrap();
    assert_eq!(
        draft.fields()[1].number_size(),
        Some(DraftNumberSize::Decimal)
    );
    assert_eq!(draft.fields()[1].decimal_constraints(), Some(constraints));
    draft
        .set_field_number_size(field, DraftNumberSize::Byte)
        .unwrap();
    draft
        .set_field_number_size(field, DraftNumberSize::Decimal)
        .unwrap();
    assert_eq!(draft.fields()[1].decimal_constraints(), Some(constraints));
}

#[test]
fn number_setters_reject_protected_inapplicable_and_exhausted_changes_atomically() {
    let mut draft = TableDraft::initial();
    let primary = draft.fields()[0].id();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let constraints = DecimalConstraints::new(18, 0).unwrap();
    let before = draft.snapshot("Numbers").unwrap();
    assert_eq!(
        draft.set_field_number_size(primary, DraftNumberSize::Byte),
        Err(TableDraftError::ProtectedField)
    );
    assert_eq!(
        draft.set_field_decimal_constraints(primary, constraints),
        Err(TableDraftError::ProtectedField)
    );
    assert_eq!(
        draft.set_field_number_size(field, DraftNumberSize::Byte),
        Err(TableDraftError::InvalidProperty)
    );
    assert_eq!(
        draft.set_field_decimal_constraints(field, constraints),
        Err(TableDraftError::InvalidProperty)
    );
    assert!(draft.snapshot("Numbers").unwrap() == before);

    draft.set_field_type(field, DraftFieldType::Number).unwrap();
    draft
        .set_field_number_size(field, DraftNumberSize::Decimal)
        .unwrap();
    draft.revision = u64::MAX;
    let before = draft.snapshot("Numbers").unwrap();
    assert_eq!(
        draft.set_field_number_size(field, DraftNumberSize::Byte),
        Err(TableDraftError::Limit)
    );
    assert_eq!(
        draft.set_field_decimal_constraints(field, DecimalConstraints::new(12, 2).unwrap()),
        Err(TableDraftError::Limit)
    );
    assert!(draft.snapshot("Numbers").unwrap() == before);
}
