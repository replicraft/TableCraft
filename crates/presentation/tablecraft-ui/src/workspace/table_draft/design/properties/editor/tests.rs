use super::*;

#[test]
fn exhausted_property_revision_refuses_every_edit_without_touching_typed_or_raw_input() {
    let mut draft = TableDraft::initial();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let mut editors = Editors::default();
    let position = editors.ensure(&draft, field).unwrap();
    let snapshot = draft.snapshot("Items").unwrap();
    editors.revision = u64::MAX;
    assert!(!editors.set_size(&mut draft, position, "24".into(), false));
    assert!(!editors.set_required(&mut draft, position, Some(true)));
    assert!(!editors.set_zero_length(&mut draft, position, Some(false)));
    assert!(!editors.set_mode(&mut draft, position, DefaultChoice::Null));
    assert!(!editors.set_literal(&mut draft, position, "kept out".into(), false));
    assert!(draft.snapshot("Items").unwrap() == snapshot);
    assert_eq!(editors.size(position), "255");
    assert_eq!(editors.literal(position), "");
    assert!(!editors.rejected());
}

#[test]
fn protected_id_refuses_even_invalid_or_oversized_property_text_without_poisoning_cache() {
    let mut draft = TableDraft::initial();
    let primary = draft.fields()[0].id();
    let mut editors = Editors::default();
    let position = editors.ensure(&draft, primary).unwrap();
    assert!(!editors.set_size(&mut draft, position, "invalid".into(), false));
    assert!(!editors.set_required(&mut draft, position, Some(false)));
    assert!(!editors.set_zero_length(&mut draft, position, Some(true)));
    assert!(!editors.set_mode(&mut draft, position, DefaultChoice::Absent));
    assert!(!editors.set_literal(&mut draft, position, "x".repeat(4097), false));
    assert_eq!(editors.revision(), 0);
    assert!(!editors.rejected());
    assert!(!draft.is_modified());
}
