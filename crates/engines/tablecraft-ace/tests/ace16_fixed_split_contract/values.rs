use super::*;

const KINDS: [Kind; 4] = [Kind::Byte, Kind::Int16, Kind::Int64, Kind::Currency];

fn run(model: &Model) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    root_model::capture(&model.bytes).preview_ace16_split_fixed_updates(
        "Items",
        "Score",
        &[request(model)],
        &mut budget(),
        &|| false,
    )
}

#[test]
fn root_splits_use_exact_declared_width_both_directions_aliases_and_full_row_ties() {
    for code in [5, 6] {
        for kind in KINDS {
            let model = source(code, kind, vec![kind.root_count()], false);
            check(&model);
            for physical in 0..2 {
                let keys = ordered(&model, &model.values, physical == 1);
                assert_eq!(&keys[0][..kind.width() + 1], &keys[1][..kind.width() + 1]);
                assert_ne!(&keys[0][kind.width() + 1..], &keys[1][kind.width() + 1..]);
            }
            let preview = run(&model).unwrap();
            assert_eq!(preview.candidate_bytes(), expected(&model));
            assert_eq!(
                preview.candidate_bytes().len(),
                model.bytes.len() + 4 * PAGE
            );
        }
    }
}

#[test]
fn stored_null_peers_retain_masks_payload_and_membership_without_null_requests() {
    for kind in KINDS {
        let model = source(6, kind, vec![100], true);
        check(&model);
        let preview = run(&model).unwrap();
        assert_eq!(preview.candidate_bytes(), expected(&model));
        let split = source(6, kind, vec![kind.root_count(), 1], true);
        check(&split);
        let preview = run(&split).unwrap();
        assert_eq!(preview.candidate_bytes(), expected(&split));
        assert_eq!(
            preview.candidate_bytes().len(),
            split.bytes.len() + 2 * PAGE
        );

        let none = FixedScalarUpdate::new(0, Value::Null, kind.value(1));
        assert!(
            root_model::capture(&model.bytes)
                .preview_ace16_split_fixed_updates(
                    "Items",
                    "Score",
                    &[none],
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
        let none = FixedScalarUpdate::new(0, kind.value(0), Value::Null);
        assert!(
            root_model::capture(&model.bytes)
                .preview_ace16_split_fixed_updates(
                    "Items",
                    "Score",
                    &[none],
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
    }
}

#[test]
fn nonroot_overflow_retains_right_leaf_repairs_siblings_and_composes_owner_map_bytes() {
    for kind in KINDS {
        let dense = kind.root_count();
        let model = source(6, kind, vec![dense, 1, 1], false);
        check(&model);
        let preview = run(&model).unwrap();
        assert_eq!(preview.candidate_bytes(), expected(&model));
        assert_eq!(
            preview.candidate_bytes().len(),
            model.bytes.len() + 2 * PAGE
        );
    }
}

#[test]
fn genuine_best_prefix_parent_overflow_grows_roots_without_leaf_overflow() {
    for kind in KINDS {
        let bounds = match kind {
            Kind::Byte => 602,
            Kind::Int16 => 601,
            Kind::Int64 | Kind::Currency => 600,
            Kind::Int32 => unreachable!(),
        };
        let mut model = source(5, kind, vec![1; bounds], false);
        check(&model);
        let keys = ordered(&model, &model.values, false);
        let original_bounds = keys
            .iter()
            .enumerate()
            .map(|(position, key)| bound(key, model.leaves[0][position]))
            .collect::<Vec<_>>();
        let best = common(&original_bounds, true);
        assert_eq!(best, kind.width() + 3);
        assert!(encoded_len(&original_bounds, best) <= PAGE - ENTRY);
        if matches!(kind, Kind::Int64 | Kind::Currency) {
            // The exact original MAX request cannot fit its descending new
            // left half: 299 bounds require 4,785 bytes even at best prefix.
            // Keep that refusal and the same 600-child source. A one-unit
            // replacement still overflows the parent, while both halves fit.
            let original = model.bytes.clone();
            assert_eq!(
                run(&model).err(),
                Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
            );
            assert_eq!(model.bytes, original);
            model.replacement = 1;
        }
        let mut changed = model.values.clone();
        changed[0] = Some(model.replacement);
        let changed_keys = ordered(&model, &changed, false);
        let changed_bounds = changed_keys
            .iter()
            .enumerate()
            .map(|(position, key)| bound(key, model.leaves[0][position]))
            .collect::<Vec<_>>();
        assert!(encoded_len(&changed_bounds, common(&changed_bounds, true)) > PAGE - ENTRY);
        for physical in 0..2 {
            let changed_keys = ordered(&model, &changed, physical == 1);
            let boundaries = changed_keys
                .iter()
                .enumerate()
                .map(|(position, key)| bound(key, model.leaves[physical][position]))
                .collect::<Vec<_>>();
            let middle = boundaries.len() / 2;
            for half in [
                &boundaries[..middle - 1],
                &boundaries[middle..boundaries.len() - 1],
            ] {
                assert!(encoded_len(half, common(half, true)) <= PAGE - ENTRY);
            }
        }
        let preview = run(&model).unwrap();
        assert_eq!(preview.candidate_bytes(), expected(&model));
        assert_eq!(
            preview.candidate_bytes().len(),
            model.bytes.len() + 4 * PAGE
        );
    }
}

#[test]
fn currency_and_int64_authority_is_declared_not_inferred_from_equal_width() {
    for kind in [Kind::Currency, Kind::Int64] {
        let model = source(6, kind, vec![400], false);
        check(&model);
        let wrong = match kind {
            Kind::Currency => Kind::Int64,
            Kind::Int64 => Kind::Currency,
            _ => unreachable!(),
        };
        for request in [
            FixedScalarUpdate::new(0, wrong.value(0), kind.value(kind.maximum())),
            FixedScalarUpdate::new(0, kind.value(0), wrong.value(kind.maximum())),
        ] {
            assert!(matches!(
                root_model::capture(&model.bytes).preview_ace16_split_fixed_updates(
                    "Items",
                    "Score",
                    &[request],
                    &mut budget(),
                    &|| false,
                ),
                Err(EditError::Unsupported(WriteBlocker::Column))
            ));
        }
        assert_eq!(run(&model).unwrap().candidate_bytes(), expected(&model));
    }
}

#[test]
fn old_int32_facade_agrees_and_resource_failures_leave_retry_unchanged() {
    let model = source(6, Kind::Int32, vec![500], false);
    check(&model);
    let old = root_model::capture(&model.bytes)
        .preview_ace16_split_int32_updates(
            "Items",
            "Score",
            &[Int32Update::new(0, 0, i32::MAX)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        run(&model).unwrap().candidate_bytes(),
        old.candidate_bytes()
    );
    for limits in [
        (1, 1_000_000, 100_000_000),
        (256 * 1024 * 1024, 1, 100_000_000),
        (256 * 1024 * 1024, 1_000_000, 1),
    ] {
        assert!(
            root_model::capture(&model.bytes)
                .preview_ace16_split_fixed_updates(
                    "Items",
                    "Score",
                    &[request(&model)],
                    &mut ResourceBudget::new(limits.0, limits.1, limits.2),
                    &|| false,
                )
                .is_err()
        );
    }
    let calls = Cell::new(0);
    assert!(
        root_model::capture(&model.bytes)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &[request(&model)],
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == 200
                },
            )
            .is_err()
    );
    assert_eq!(calls.get(), 200);
    assert_eq!(run(&model).unwrap().candidate_bytes(), expected(&model));
}

#[test]
fn exact_expected_and_duplicate_root_requests_refuse_before_atomic_publication() {
    for kind in KINDS {
        let model = source(6, kind, vec![kind.root_count()], false);
        check(&model);
        let stale = FixedScalarUpdate::new(0, kind.value(1), kind.value(kind.maximum()));
        assert!(matches!(
            root_model::capture(&model.bytes).preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &[stale],
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Conflict)
        ));
        assert!(matches!(
            root_model::capture(&model.bytes).preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &[request(&model), request(&model)],
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Conflict)
        ));
        assert_eq!(run(&model).unwrap().candidate_bytes(), expected(&model));
    }
}

#[test]
fn represented_map_authority_is_required_for_every_new_owner_page() {
    for kind in KINDS {
        for mode in 0..3 {
            let mut model = source(6, kind, vec![kind.root_count()], false);
            let error = if mode == 1 {
                InspectError::Malformed
            } else {
                InspectError::Unsupported
            };
            if mode == 0 {
                let payload = vec![0; 8];
                install(&mut model.bytes, 1, &support::data_page(1, &[(0, payload)]));
            } else if mode == 1 {
                let offset = root_model::map_byte(&model.bytes, 1, model.eof);
                model.bytes[offset] &= !(1 << (model.eof % 8));
            } else {
                let mut payload = vec![0; 8];
                payload[6] = 1 << (ROOTS[0] % 8);
                install(
                    &mut model.bytes,
                    MAPS[0],
                    &support::data_page(0, &[(0, payload)]),
                );
            }
            check(&model);
            let actual = run(&model).err().unwrap();
            assert!(matches!(
                (actual, error),
                (
                    EditError::Inspect(InspectError::Malformed),
                    InspectError::Malformed
                ) | (
                    EditError::Inspect(InspectError::Unsupported),
                    InspectError::Unsupported
                )
            ));
        }
    }
}
