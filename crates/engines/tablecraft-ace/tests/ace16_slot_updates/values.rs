use super::*;

#[test]
fn text_single_indexes_reencode_new_keys_and_preserve_payload_width() {
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
                        Value::Text("é".into()),
                    )],
                )
                .unwrap();
                assert_eq!(output.before_bytes(), model.bytes);
                assert_eq!(output.changed_rows(), 1);
                assert_rows(&model, output.candidate_bytes(), 2, Value::Text("é".into()));
                let mut expected = model.keys[4].clone();
                expected.extend(((12_u32 << 8) | 2).to_be_bytes());
                let actual = entries(output.candidate_bytes(), 13);
                assert!(actual.contains(&expected));
                assert!(!actual.contains(&wire(&model, 2)));
                for row in (0..model.records.len()).filter(|row| *row != 2) {
                    assert!(actual.contains(&wire(&model, row)));
                }
            }
        }
    }
}

#[test]
fn binary_segmented_indexes_use_real_replacement_encoding_in_both_directions() {
    for descending in [false, true] {
        let model = model::binary(descending, true);
        let output = preview(
            &model,
            &[SlotScalarUpdate::new(
                4,
                Value::Binary((1..=8).collect()),
                Value::Binary(vec![255; 8]),
            )],
        )
        .unwrap();
        assert_rows(
            &model,
            output.candidate_bytes(),
            4,
            Value::Binary(vec![255; 8]),
        );
        let mut expected = model.keys[6].clone();
        expected.extend(((12_u32 << 8) | 4).to_be_bytes());
        assert!(entries(output.candidate_bytes(), 13).contains(&expected));
    }
}

#[test]
fn tree_replacements_recompute_full_ancestor_keys_without_changing_topology() {
    for bounded in [false, true] {
        let model = model::tree(model::text(1, false, true), bounded);
        let output = preview(
            &model,
            &[SlotScalarUpdate::new(
                2,
                Value::Text("a".into()),
                Value::Text("é".into()),
            )],
        )
        .unwrap();
        assert_rows(&model, output.candidate_bytes(), 2, Value::Text("é".into()));
        for page in [13, 15, 16, 19] {
            let start = page * 4096;
            // Kind/owner/links/height and original tail topology are immutable.
            assert_eq!(
                &output.candidate_bytes()[start..start + 2],
                &model.bytes[start..start + 2]
            );
            assert_eq!(
                &output.candidate_bytes()[start + 4..start + 24],
                &model.bytes[start + 4..start + 24]
            );
            assert_eq!(
                output.candidate_bytes()[start + 26],
                model.bytes[start + 26]
            );
        }
    }
}

#[test]
fn mixed_tuple_replacement_encodes_all_components_and_preserves_row_count() {
    let model = model::composite();
    let output = preview(
        &model,
        &[SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text("é".into()),
        )],
    )
    .unwrap();
    assert_rows(&model, output.candidate_bytes(), 2, Value::Text("é".into()));
    let mut expected = model.keys[4][..model.keys[4].len() - 5].to_vec();
    expected.extend([127, 128, 0, 0, 3]);
    expected.extend(((12_u32 << 8) | 2).to_be_bytes());
    assert!(entries(output.candidate_bytes(), 13).contains(&expected));
}

#[test]
fn null_empty_and_noop_distinctions_preserve_existing_slot_bytes() {
    for (model, empty) in [
        (model::text(1, false, false), Value::Text(String::new())),
        (model::binary(false, false), Value::Binary(Vec::new())),
    ] {
        let output = preview(
            &model,
            &[SlotScalarUpdate::new(0, Value::Null, empty.clone())],
        )
        .unwrap();
        assert_rows(&model, output.candidate_bytes(), 0, empty.clone());
        let output = preview(&model, &[SlotScalarUpdate::new(1, empty, Value::Null)]).unwrap();
        assert_rows(&model, output.candidate_bytes(), 1, Value::Null);
        assert_eq!(preview(&model, &[]).unwrap().candidate_bytes(), model.bytes);
    }
}

#[test]
fn compression_preference_does_not_hide_valid_existing_span_representations() {
    let mut model = model::text(1, false, true);
    // Literal extended flag on the independently authored Score descriptor.
    model.bytes[10 * 4096 + 116] = 1;
    let fallback = preview(
        &model,
        &[SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text("A".into()),
        )],
    )
    .unwrap();
    assert_rows(
        &model,
        fallback.candidate_bytes(),
        2,
        Value::Text("A".into()),
    );

    // An observed FFFE marker alone represents empty in a two-byte slot.
    let empty = preview(
        &model,
        &[SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text(String::new()),
        )],
    )
    .unwrap();
    assert_rows(
        &model,
        empty.candidate_bytes(),
        2,
        Value::Text(String::new()),
    );
    let after = empty.candidate_bytes();
    let start = usize::from(
        u16::from_le_bytes(after[12 * 4096 + 18..12 * 4096 + 20].try_into().unwrap()) & 0x1fff,
    );
    assert_eq!(
        &after[12 * 4096 + start + 6..12 * 4096 + start + 8],
        &[255, 254]
    );
    let unchanged = capture(after)
        .preview_ace16_slot_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text(String::new()),
                Value::Text(String::new()),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(unchanged.candidate_bytes(), after);

    let null = capture(after)
        .preview_ace16_slot_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                2,
                Value::Text(String::new()),
                Value::Null,
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        &null.candidate_bytes()[12 * 4096 + start + 6..12 * 4096 + start + 8],
        &[255, 254]
    );
    assert_rows(&model, null.candidate_bytes(), 2, Value::Null);
}
