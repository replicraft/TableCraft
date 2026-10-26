use super::model::*;
use tablecraft_ace::{EditError, WriteBlocker};

fn root_cases(original: Model) {
    for code in [5, 6] {
        let mut model = Model {
            bytes: original.bytes.clone(),
            records: original.records.clone(),
            keys: original.keys.clone(),
            order: original.order.clone(),
        };
        model.bytes[20] = code;
        assert_eq!(
            candidate(&model, &[]).unwrap().candidate_bytes(),
            model.bytes
        );
        for deleted in [
            vec![model.order[0]],
            vec![model.order[4]],
            vec![*model.order.last().unwrap()],
            (0..model.records.len()).collect(),
        ] {
            let output = candidate(&model, &deleted).unwrap();
            assert_eq!(output.before_bytes(), model.bytes);
            assert_eq!(output.deleted_rows(), deleted.len());
            let expected = model
                .order
                .iter()
                .copied()
                .filter(|row| !deleted.contains(row))
                .map(|row| wire(&model, row))
                .collect::<Vec<_>>();
            assert_eq!(entries(output.candidate_bytes(), 13), expected);
            verify_rows(&model, output.candidate_bytes(), &deleted);
            let mut allowed = vec![false; model.bytes.len()];
            allowed[10 * 4096 + 16..10 * 4096 + 20].fill(true);
            for row in &deleted {
                allowed[12 * 4096 + 14 + row * 2..12 * 4096 + 16 + row * 2].fill(true);
            }
            allowed[13 * 4096 + 2..13 * 4096 + 4].fill(true);
            allowed[13 * 4096 + 24..13 * 4096 + 26].fill(true);
            allowed[13 * 4096 + 27..14 * 4096].fill(true);
            for (offset, (&before, &after)) in
                model.bytes.iter().zip(output.candidate_bytes()).enumerate()
            {
                assert!(
                    allowed[offset] || before == after,
                    "unexpected byte {offset}"
                );
            }
        }
    }
}

#[test]
fn root_leaf_text_keeps_exact_general_weights_and_overflow_digest_bytes() {
    for family in [0, 1] {
        for descending in [false, true] {
            for alias in [false, true] {
                root_cases(text(family, descending, alias));
            }
        }
    }
    for descending in [false, true] {
        let model = text(1, descending, true);
        assert_eq!(model.keys[8].len(), 510);
        let output = candidate(&model, &[2]).unwrap();
        let actual = entries(output.candidate_bytes(), 13);
        assert!(actual.contains(&wire(&model, 8)));
        assert_eq!(
            &model.keys[8][508..],
            if descending {
                &[0xf7, 0x5b]
            } else {
                &[0x27, 0xa5]
            }
        );
    }
}

#[test]
fn binary_segment_boundaries_padding_null_and_empty_keep_original_wire_entries() {
    for descending in [false, true] {
        for alias in [false, true] {
            root_cases(binary(descending, alias));
        }
    }
}

fn tree_case(model: Model, bounded: bool, contract: bool) {
    let model = tree(model, bounded);
    let deleted = if contract {
        model.order[..3].to_vec()
    } else {
        vec![model.order[4]]
    };
    let output = candidate(&model, &deleted).unwrap();
    let after = output.candidate_bytes();
    verify_rows(&model, after, &deleted);
    let mut expected_bounds = Vec::new();
    for (position, rows) in model.order.chunks(3).enumerate() {
        let number = [15_u32, 16, 19][position];
        let expected = rows
            .iter()
            .copied()
            .filter(|row| !deleted.contains(row))
            .map(|row| wire(&model, row))
            .collect::<Vec<_>>();
        if expected.is_empty() {
            assert_eq!(
                &after[number as usize * 4096..number as usize * 4096 + 4],
                &[0; 4]
            );
            assert_eq!(
                &after[number as usize * 4096 + 4..(number as usize + 1) * 4096],
                &model.bytes[number as usize * 4096 + 4..(number as usize + 1) * 4096]
            );
            continue;
        }
        assert_eq!(entries(after, number as usize), expected);
        if bounded || position < 2 {
            let mut maximum = expected.last().unwrap().clone();
            maximum.extend(number.to_be_bytes());
            expected_bounds.push(maximum);
        }
    }
    assert_eq!(entries(after, 13), expected_bounds);
    assert_eq!(after[13 * 4096], 3);
    assert_eq!(after[13 * 4096 + 26], 1);
    assert_eq!(
        &after[10 * 4096 + 67..10 * 4096 + 71],
        &model.bytes[10 * 4096 + 67..10 * 4096 + 71]
    );
    if contract {
        assert_eq!(after[15 * 4096 - 2], 0x20);
        assert_eq!(after[8192 - 64 + 1], 0x80);
        assert_eq!(&after[16 * 4096 + 12..16 * 4096 + 16], &[0; 4]);
    }
}

#[test]
fn text_and_binary_node_maxima_use_full_original_key_and_row_boundaries() {
    for descending in [false, true] {
        for bounded in [false, true] {
            tree_case(text(1, descending, true), bounded, false);
            tree_case(binary(descending, true), bounded, false);
        }
    }
}

#[test]
fn encoded_entry_leaf_retirement_reuses_checked_maps_without_changing_root_height() {
    for descending in [false, true] {
        for bounded in [false, true] {
            tree_case(text(1, descending, true), bounded, true);
            tree_case(binary(descending, true), bounded, true);
        }
    }
}

#[test]
fn deletion_admission_does_not_enable_indexed_text_or_binary_update_calls() {
    for model in [text(1, false, false), binary(false, false)] {
        assert!(candidate(&model, &[0]).is_ok());
        assert!(matches!(
            capture(&model.bytes).preview_ace16_int32_updates(
                "Items",
                "Score",
                &[],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(
                WriteBlocker::Column | WriteBlocker::ConstraintCoverage
            ))
        ));
    }
    // Readable single Short Binary still refuses ordinary Jet4 mutation. The
    // new delete-only ACE preview must not weaken that existing writer gate.
    let mut model = binary(false, false);
    model.bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    model.bytes[20] = 1;
    assert!(matches!(
        capture(&model.bytes).plan_int32_updates("Items", "Id", &[], &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));
}
