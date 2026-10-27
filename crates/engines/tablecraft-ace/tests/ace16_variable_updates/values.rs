use super::*;

#[test]
fn indexed_text_root_and_node_updates_use_alternate_or_eof_and_real_alias_keys() {
    for code in [5, 6] {
        for family in [0, 1] {
            for descending in [false, true] {
                for node in [None, Some(false), Some(true)] {
                    for alternate in [false, true] {
                        let mut original = page_model::text(family, descending, true);
                        if let Some(bounded) = node {
                            original = page_model::tree(original, bounded);
                        }
                        let mut model = source(original, alternate);
                        model.bytes[20] = code;
                        check_table(
                            &mut Cursor::new(&model.bytes),
                            CatalogScope::new(1),
                            "Items",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let output =
                            preview(&model, 2, Value::Text("a".into()), Value::Text("ss".into()))
                                .unwrap();
                        let after = output.candidate_bytes();
                        assert_eq!(output.changed_rows(), 1);
                        assert_eq!(output.before_bytes(), model.bytes);
                        assert_value(after, 2, Value::Text("ss".into()));
                        let target_page = if alternate { 18 } else { 20 };
                        let target_slot = u32::from(alternate);
                        let pointer =
                            u32::from_le_bytes(slot_body(after, 12, 2)[..4].try_into().unwrap());
                        assert_eq!(pointer, (target_page << 8) | target_slot);
                        assert_eq!(
                            after.len(),
                            model.bytes.len() + if alternate { 0 } else { 4096 }
                        );
                        let mut expected = model.keys[7].clone();
                        expected.extend(((12_u32 << 8) | 2).to_be_bytes());
                        let pages = if node.is_some() {
                            vec![15, 16, 19]
                        } else {
                            vec![13]
                        };
                        let actual = pages
                            .into_iter()
                            .flat_map(|page| entries(after, page))
                            .collect::<Vec<_>>();
                        assert!(actual.contains(&expected));
                        assert!(!actual.contains(&wire(&model, 2)));
                        assert_eq!(
                            &after[10 * 4096..11 * 4096],
                            &model.bytes[10 * 4096..11 * 4096]
                        );
                        for row in (0..model.records.len()).filter(|row| *row != 2) {
                            assert_eq!(slot_body(after, 12, row), model.records[row]);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn binary_segment_change_and_mixed_tuple_replace_wire_with_original_root_reference() {
    for descending in [false, true] {
        let model = source(page_model::binary(descending, true), false);
        let output = preview(
            &model,
            4,
            Value::Binary((1..=8).collect()),
            Value::Binary((1..=9).collect()),
        )
        .unwrap();
        assert_value(
            output.candidate_bytes(),
            4,
            Value::Binary((1..=9).collect()),
        );
        let mut expected = model.keys[5].clone();
        expected.extend(((12_u32 << 8) | 4).to_be_bytes());
        assert!(entries(output.candidate_bytes(), 13).contains(&expected));
    }
    for alternate in [false, true] {
        let model = source(page_model::composite(), alternate);
        let output = preview(&model, 2, Value::Text("a".into()), Value::Text("ss".into())).unwrap();
        assert_value(output.candidate_bytes(), 2, Value::Text("ss".into()));
        let mut expected = model.keys[7][..model.keys[7].len() - 5].to_vec();
        expected.extend([127, 128, 0, 0, 3]);
        expected.extend(((12_u32 << 8) | 2).to_be_bytes());
        assert!(entries(output.candidate_bytes(), 13).contains(&expected));
    }
}

#[test]
fn forwarded_history_retains_old_hidden_bodies_shrink_capacity_and_noop_bytes() {
    let model = source(page_model::text(1, false, true), false);
    let first = preview(&model, 2, Value::Text("a".into()), Value::Text("ss".into())).unwrap();
    let old_hidden = slot_body(first.candidate_bytes(), 20, 0).to_vec();
    let second = capture(first.candidate_bytes())
        .preview_ace16_variable_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text("ss".into()),
                Value::Text("ééé".into()),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        second.candidate_bytes().len(),
        first.candidate_bytes().len()
    );
    assert_eq!(slot_body(second.candidate_bytes(), 20, 0), old_hidden);
    assert_value(second.candidate_bytes(), 2, Value::Text("ééé".into()));
    let third = capture(second.candidate_bytes())
        .preview_ace16_variable_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text("ééé".into()),
                Value::Text("b".into()),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(slot_body(third.candidate_bytes(), 20, 0), old_hidden);
    assert_eq!(
        slot_body(third.candidate_bytes(), 20, 1).len(),
        slot_body(second.candidate_bytes(), 20, 1).len()
    );
    assert_value(third.candidate_bytes(), 2, Value::Text("b".into()));
    let noop = capture(third.candidate_bytes())
        .preview_ace16_variable_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text("b".into()),
                Value::Text("b".into()),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(noop.changed_rows(), 0);
    assert_eq!(noop.candidate_bytes(), third.candidate_bytes());
}

#[test]
fn allocated_or_reused_targets_still_require_final_field_validation() {
    use tablecraft_ace::{ConstraintViolation, InspectError, check_table_constraints};

    for alternate in [false, true] {
        let mut model = source(page_model::text(1, false, true), alternate);
        let catalog = fixture::with_rule("ValidationRule", "Len([Score] & '') <> 3");
        model.bytes[4 * 4096..5 * 4096].copy_from_slice(&catalog[4 * 4096..5 * 4096]);
        check_table_constraints(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let result = preview(
            &model,
            2,
            Value::Text("a".into()),
            Value::Text("ABC".into()),
        );
        assert_eq!(
            result.err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::FieldRule
            )))
        );
    }
}
