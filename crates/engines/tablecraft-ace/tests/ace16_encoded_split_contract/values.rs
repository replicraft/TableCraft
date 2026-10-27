use super::*;
use model::Family;
use tablecraft_ace::WriteBlocker;

#[test]
fn root_overflow_preserves_six_complete_encoded_images() {
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        for descending in [false, true] {
            let mut case = model::root_case(family, descending);
            for header in [5, 6] {
                case.bytes[20] = header;
                case.expected[20] = header;
                accepted(&case);
                let old = encoded_model::capture(&case.bytes)
                    .preview_ace16_slot_updates(
                        "Items",
                        "Score",
                        &[SlotScalarUpdate::new(
                            case.row,
                            case.old.clone(),
                            case.new.clone(),
                        )],
                        &mut budget(),
                        &|| false,
                    )
                    .err();
                assert_eq!(
                    old,
                    Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
                );
            }
        }
    }
}

#[test]
fn null_empty_and_opaque_slots_keep_distinct_presence_masks() {
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        for descending in [false, true] {
            let source = model::small_source(family, descending);
            let empty = if matches!(family, Family::Binary) {
                Value::Binary(vec![])
            } else {
                Value::Text("".into())
            };
            let case = model::small_case(
                source,
                0,
                Value::Null,
                empty.clone(),
                model::small_source(family, descending).keys[1].clone(),
            );
            accepted(&case);
            let source = model::small_source(family, descending);
            let null_key = source.keys[0].clone();
            accepted(&model::small_case(
                source,
                1,
                empty,
                Value::Null,
                null_key.clone(),
            ));
            let mut residue = model::small_source(family, descending);
            let payload = if matches!(family, Family::Binary) {
                vec![0]
            } else {
                b"a\0".to_vec()
            };
            let present_key = residue.keys[2].clone();
            model::rewrite_small_source(&mut residue, 0, &payload, null_key, false);
            let present = if matches!(family, Family::Binary) {
                Value::Binary(vec![0])
            } else {
                Value::Text("a".into())
            };
            accepted(&model::small_case(
                residue,
                0,
                Value::Null,
                present,
                present_key,
            ));
            let root = model::root_case(family, descending);
            let noop = preview(
                &root.bytes,
                &[SlotScalarUpdate::new(0, root.old.clone(), root.old)],
            )
            .unwrap();
            assert_eq!(noop.changed_rows(), 0);
            assert_eq!(noop.candidate_bytes(), root.bytes);
        }
    }
}

#[test]
fn equal_weights_and_trailing_spaces_preserve_stored_values_and_row_ties() {
    for family in [Family::Legacy, Family::Modern] {
        for descending in [false, true] {
            let source = model::small_source(family, descending);
            assert_eq!(source.keys[2], source.keys[3]);
            assert_eq!(source.keys[4], source.keys[5]);
            assert_eq!(source.keys[6], source.keys[7]);
            let same = source.keys[3].clone();
            accepted(&model::small_case(
                source,
                2,
                Value::Text("a".into()),
                Value::Text("A".into()),
                same,
            ));
            let mut trailing = model::small_source(family, descending);
            let key = trailing.keys[2].clone();
            // The exact a-two-spaces vector is tracked for both families.
            model::rewrite_small_source(&mut trailing, 2, b"a\0 \0 \0", key.clone(), true);
            accepted(&model::small_case(
                trailing,
                2,
                Value::Text("a  ".into()),
                Value::Text("A  ".into()),
                key,
            ));
        }
    }
}

#[test]
fn binary_segment_boundaries_keep_exact_direction_markers_and_overflow() {
    for descending in [false, true] {
        for width in [0, 1, 7, 8, 9, 16, 17, 255] {
            let case = model::uniform_case(Family::Binary, descending, width, false);
            accepted(&case);
            if width == 255 {
                assert_eq!(case.expected.len(), case.bytes.len() + 8192);
            }
            let key = model::binary_key(&vec![0; width], descending);
            if width > 8 {
                assert_eq!(key[9], 9);
            }
            if width > 0 {
                assert_eq!(
                    *key.last().unwrap(),
                    if descending {
                        !(((width - 1) % 8 + 1) as u8)
                    } else {
                        ((width - 1) % 8 + 1) as u8
                    }
                );
            }
        }
    }
}

#[test]
fn maximum_text_digest_wires_survive_null_updates_and_internal_root_growth() {
    for family in [Family::Legacy, Family::Modern] {
        for descending in [false, true] {
            let case = model::uniform_case(family, descending, 255, true);
            accepted(&case);
            assert_eq!(case.expected.len(), case.bytes.len() + 8192);
        }
    }
    let parent = model::maximum_parent_case();
    accepted(&parent);
    assert_eq!(parent.bytes[13 * 4096 + 26], 1);
    assert_eq!(parent.expected[13 * 4096 + 26], 2);
    // This deliberately claims no independent new non-NULL digest oracle.
    let mut unique = encoded_model::single_overflow();
    unique.bytes[10 * 4096 + 115] = 2;
    let before = unique.bytes.clone();
    for requests in [
        vec![],
        vec![SlotScalarUpdate::new(
            0,
            Value::Text("a".repeat(255)),
            Value::Text("a".repeat(255)),
        )],
    ] {
        let old = encoded_model::capture(&before)
            .preview_ace16_slot_updates("Items", "Score", &requests, &mut budget(), &|| false)
            .err();
        assert!(old.is_some());
        assert_eq!(preview(&before, &requests).err(), old);
    }
}

#[test]
fn nonroot_split_with_null_peer_repairs_all_siblings_and_bounded_ancestors() {
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        for descending in [false, true] {
            for height_two in [false, true] {
                let case = model::nonroot_case(family, descending, height_two);
                accepted(&case);
                assert_eq!(case.expected.len(), case.bytes.len() + 4096);
                assert_eq!(
                    case.expected[13 * 4096 + 26],
                    if height_two { 2 } else { 1 }
                );
            }
        }
    }
}

#[test]
fn two_physical_owners_and_true_alias_compose_the_same_global_byte() {
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        let promoted = model::promotion_case(family);
        accepted(&promoted);
        assert_eq!(promoted.expected.len(), promoted.bytes.len() + 4 * 4096);
        assert_eq!(
            &promoted.expected[601 * 4096..601 * 4096 + 4],
            &[5, 1, 0, 0]
        );
        let case = model::two_owners(family);
        accepted(&case);
        assert_eq!(case.expected.len(), case.bytes.len() + 6 * 4096);
        assert_eq!(case.bytes[4096 + 4027 + 5 + 3], 255);
        assert_eq!(case.expected[4096 + 4027 + 5 + 3], 192);
        let report = check_table(
            &mut Cursor::new(&case.bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.indexes().len(), 3);
        assert_eq!(
            report.indexes()[0].index().data_number(),
            report.indexes()[1].index().data_number()
        );
        assert_ne!(
            report.indexes()[0].index().data_number(),
            report.indexes()[2].index().data_number()
        );
    }
}
