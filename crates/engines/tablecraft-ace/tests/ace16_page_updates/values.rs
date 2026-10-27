use super::*;
use tablecraft_ace::{ConstraintViolation, InspectError, WriteBlocker, check_table_constraints};

#[test]
fn ordinary_text_growth_preserves_logical_root_keys_and_every_other_record() {
    for code in [5, 6] {
        for family in [0, 1] {
            for descending in [false, true] {
                let mut model = model::text(family, descending, true);
                model.bytes[20] = code;
                let output = preview(
                    &model,
                    &[SlotScalarUpdate::new(
                        2,
                        Value::Text("a".into()),
                        Value::Text("ss".into()),
                    )],
                )
                .unwrap();
                let after = output.candidate_bytes();
                assert_eq!(output.changed_rows(), 1);
                assert_eq!(output.before_bytes(), model.bytes);
                assert_value(after, 2, Value::Text("ss".into()));
                let before_word = word(&model.bytes, 12 * 4096 + 18);
                assert_eq!(word(after, 12 * 4096 + 18), before_word | 0x4000);
                assert_eq!(word(after, 12 * 4096 + 12), 10);
                assert_eq!(word(after, 12 * 4096 + 32) & 0xe000, 0x8000);
                assert_eq!(
                    u32::from_le_bytes(raw_row(after, 2)[..4].try_into().unwrap()),
                    (12 << 8) | 9
                );
                assert!(raw_row(after, 2)[4..].iter().all(|byte| *byte == 0));
                for row in (0..9).filter(|row| *row != 2) {
                    assert_eq!(raw_row(after, row), raw_row(&model.bytes, row));
                    assert_eq!(
                        word(after, 12 * 4096 + 14 + 2 * row),
                        word(&model.bytes, 12 * 4096 + 14 + 2 * row)
                    );
                }
                let mut expected = model.keys[7].clone();
                expected.extend(((12_u32 << 8) | 2).to_be_bytes());
                assert!(entries(after, 13).contains(&expected));
                assert!(!entries(after, 13).contains(&wire(&model, 2)));
                assert_eq!(
                    &after[10 * 4096..11 * 4096],
                    &model.bytes[10 * 4096..11 * 4096]
                );
                assert_eq!(
                    &after[11 * 4096..12 * 4096],
                    &model.bytes[11 * 4096..12 * 4096]
                );
            }
        }
    }
}

#[test]
fn binary_growth_uses_segment_encoder_and_original_row_reference() {
    for descending in [false, true] {
        let model = model::binary(descending, true);
        let output = preview(
            &model,
            &[SlotScalarUpdate::new(
                4,
                Value::Binary((1..=8).collect()),
                Value::Binary((1..=9).collect()),
            )],
        )
        .unwrap();
        let after = output.candidate_bytes();
        assert_value(after, 4, Value::Binary((1..=9).collect()));
        let mut expected = model.keys[5].clone();
        expected.extend(((12_u32 << 8) | 4).to_be_bytes());
        assert!(entries(after, 13).contains(&expected));
        assert_eq!(
            u32::from_le_bytes(raw_row(after, 4)[..4].try_into().unwrap()),
            (12 << 8) | 9
        );
    }
}

#[test]
fn indexed_nodes_and_mixed_tuples_keep_full_ancestor_maxima() {
    for bounded in [false, true] {
        let model = model::tree(model::text(1, false, true), bounded);
        let output = preview(
            &model,
            &[SlotScalarUpdate::new(
                2,
                Value::Text("a".into()),
                Value::Text("ss".into()),
            )],
        )
        .unwrap();
        let after = output.candidate_bytes();
        assert_value(after, 2, Value::Text("ss".into()));
        let mut expected = (0..9)
            .map(|row| {
                let mut entry = if row == 2 {
                    model.keys[7].clone()
                } else {
                    model.keys[row].clone()
                };
                entry.extend(((12_u32 << 8) | row as u32).to_be_bytes());
                entry
            })
            .collect::<Vec<_>>();
        expected.sort();
        let mut actual = Vec::new();
        let mut bounds = Vec::new();
        for number in [15, 16, 19] {
            let leaf = entries(after, number);
            let mut maximum = leaf.last().unwrap().clone();
            maximum.extend((number as u32).to_be_bytes());
            bounds.push(maximum);
            actual.extend(leaf);
        }
        if !bounded {
            bounds.pop();
        }
        assert_eq!(actual, expected);
        assert_eq!(entries(after, 13), bounds);
    }
    let model = model::composite();
    let output = preview(
        &model,
        &[SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text("ss".into()),
        )],
    )
    .unwrap();
    assert_value(output.candidate_bytes(), 2, Value::Text("ss".into()));
    let mut expected = model.keys[7][..model.keys[7].len() - 5].to_vec();
    expected.extend([127, 128, 0, 0, 3]);
    expected.extend(((12_u32 << 8) | 2).to_be_bytes());
    assert!(entries(output.candidate_bytes(), 13).contains(&expected));
}

#[test]
fn forwarded_growth_retargets_and_shrink_retains_capacity_and_old_hidden_body() {
    let model = model::text(1, false, true);
    let first = preview(
        &model,
        &[SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text("ss".into()),
        )],
    )
    .unwrap();
    let prior_hidden = raw_row(first.candidate_bytes(), 9).to_vec();
    let second = capture(first.candidate_bytes())
        .preview_ace16_page_updates(
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
    assert_value(second.candidate_bytes(), 2, Value::Text("ééé".into()));
    assert_eq!(raw_row(second.candidate_bytes(), 9), prior_hidden);
    assert_eq!(
        word(second.candidate_bytes(), 12 * 4096 + 32) & 0xe000,
        0x8000
    );
    assert_eq!(word(second.candidate_bytes(), 12 * 4096 + 12), 11);
    assert_eq!(
        u32::from_le_bytes(
            raw_row(second.candidate_bytes(), 2)[..4]
                .try_into()
                .unwrap()
        ),
        (12 << 8) | 10
    );
    let capacity = raw_row(second.candidate_bytes(), 10).len();
    let third = capture(second.candidate_bytes())
        .preview_ace16_page_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text("ééé".into()),
                Value::Text("A".into()),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_value(third.candidate_bytes(), 2, Value::Text("A".into()));
    assert_eq!(raw_row(third.candidate_bytes(), 10).len(), capacity);
    assert_eq!(raw_row(third.candidate_bytes(), 9), prior_hidden);
    assert_eq!(word(third.candidate_bytes(), 12 * 4096 + 12), 11);
    assert_eq!(
        word(third.candidate_bytes(), 12 * 4096 + 2),
        word(second.candidate_bytes(), 12 * 4096 + 2)
    );
    let noop = capture(third.candidate_bytes())
        .preview_ace16_page_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text("A".into()),
                Value::Text("A".into()),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(noop.candidate_bytes(), third.candidate_bytes());
    assert!(matches!(
        capture(third.candidate_bytes()).preview_ace16_slot_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text("A".into()),
                Value::Text("B".into())
            )],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn multiple_page_appends_are_atomic_and_independent_of_request_order() {
    let model = model::text(1, false, true);
    let updates = [
        SlotScalarUpdate::new(1, Value::Text(String::new()), Value::Text("a".into())),
        SlotScalarUpdate::new(0, Value::Null, Value::Text("ss".into())),
    ];
    let output = preview(&model, &updates).unwrap();
    let after = output.candidate_bytes();
    assert_eq!(output.changed_rows(), 2);
    assert_value(after, 0, Value::Text("ss".into()));
    assert_value(after, 1, Value::Text("a".into()));
    assert_eq!(
        u32::from_le_bytes(raw_row(after, 0)[..4].try_into().unwrap()),
        (12 << 8) | 9
    );
    assert_eq!(
        u32::from_le_bytes(raw_row(after, 1)[..4].try_into().unwrap()),
        (12 << 8) | 10
    );
    let reverse = [
        SlotScalarUpdate::new(0, Value::Null, Value::Text("ss".into())),
        SlotScalarUpdate::new(1, Value::Text(String::new()), Value::Text("a".into())),
    ];
    assert_eq!(preview(&model, &reverse).unwrap().candidate_bytes(), after);
}

#[test]
fn growth_still_enforces_final_field_rule_without_rewriting_other_records() {
    let mut model = model::text(1, false, true);
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
        &[SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text("ABC".into()),
        )],
    );
    assert_eq!(
        result.err(),
        Some(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::FieldRule
        )))
    );
}
