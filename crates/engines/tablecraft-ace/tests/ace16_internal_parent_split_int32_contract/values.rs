use super::*;

#[test]
fn below_root_growth_preserves_full_images_forms_and_all_three_sibling_levels() {
    for code in [5, 6] {
        for descending in [false, true] {
            for bounded in [false, true] {
                let target = source(code, descending, bounded);
                check(&target);
                assert_eq!(target.base.bytes[ROOT * PAGE + HEIGHT], 2);
                let original = target.base.bytes.clone();
                let result = run(&target).unwrap();
                let after = result.candidate_bytes();
                assert_eq!(after, expected(&target));
                assert_eq!(after.len(), original.len() + 2 * PAGE);
                for number in [target.base.eof, target.base.eof + 1] {
                    let end = PAGE - root_model::get16(after, number * PAGE + 2) as usize;
                    assert!(end >= ENTRY);
                    assert!(
                        after[number * PAGE + end..(number + 1) * PAGE]
                            .iter()
                            .all(|byte| *byte == 0)
                    );
                }
                assert_eq!(after[ROOT * PAGE + HEIGHT], 2);
                assert_eq!(old_model::decoded(after, ROOT).len(), 3);
                assert_eq!(root_model::get32(after, ROOT * PAGE + TAIL), 0);
                let left = target.base.eof + 1;
                assert_eq!(
                    root_model::get32(after, left * PAGE + NEXT),
                    RIGHT_PARENT as u32
                );
                assert_eq!(
                    root_model::get32(after, RIGHT_PARENT * PAGE + PREVIOUS),
                    left as u32
                );
                assert_eq!(
                    root_model::get32(after, RIGHT_PARENT * PAGE + TAIL) == 0,
                    bounded
                );
                assert_ne!(root_model::get32(after, left * PAGE + TAIL), 0);
                old_model::verify_values(&target.base, after, 0, target.replacement);
                check_table(
                    &mut Cursor::new(after),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(target.base.bytes, original);
            }
        }
    }
}

#[test]
fn tied_leaf_bounds_keep_full_row_references_through_internal_partitioning() {
    for descending in [false, true] {
        let target = tied_source(descending);
        check(&target);
        let result = run(&target).unwrap();
        assert_eq!(result.candidate_bytes(), expected(&target));
        let left = old_model::decoded(result.candidate_bytes(), target.base.eof);
        let right = old_model::decoded(result.candidate_bytes(), target.base.leaves[0][0]);
        let maximum = left.last().unwrap();
        let minimum = right.first().unwrap();
        assert_eq!(&maximum[..5], &minimum[..5]);
        assert_ne!(&maximum[5..], &minimum[5..]);
        let parent_bounds = old_model::decoded(result.candidate_bytes(), target.base.eof + 1);
        let bound = parent_bounds
            .iter()
            .find(|entry| entry.ends_with(&(target.base.eof as u32).to_be_bytes()))
            .unwrap();
        assert_eq!(&bound[..9], maximum);
        old_model::verify_values(
            &target.base,
            result.candidate_bytes(),
            0,
            target.replacement,
        );
    }
}

#[test]
fn both_future_pages_require_exact_map_authority_without_partial_publication() {
    for mode in 0..3 {
        let mut target = source(6, false, false);
        let eof = target.base.eof;
        match mode {
            0 => {
                let row = vec![0; 5 + eof / 8];
                target.base.bytes[PAGE..2 * PAGE]
                    .copy_from_slice(&support::data_page(1, &[(0, row)]));
            }
            1 => {
                let byte = root_model::map_byte(&target.base.bytes, 1, eof + 1);
                target.base.bytes[byte] &= !(1 << ((eof + 1) % 8));
            }
            _ => {
                let original = old_model::decoded(&target.base.bytes, ROOT);
                assert_eq!(original.len(), 2);
                let mut owned = vec![ROOT, RIGHT_PARENT, PEER_PARENT, PEER_LEAF];
                owned.extend_from_slice(&target.base.leaves[0]);
                let mut row = old_model::map(&owned);
                row.truncate(5 + eof / 8);
                target.base.bytes[OWNED_MAP * PAGE..(OWNED_MAP + 1) * PAGE]
                    .copy_from_slice(&support::data_page(0, &[(0, row)]));
            }
        }
        check(&target);
        let source_bytes = target.base.bytes.clone();
        let expected_error = if mode == 1 {
            InspectError::Malformed
        } else {
            InspectError::Unsupported
        };
        assert_eq!(run(&target).err(), Some(EditError::Inspect(expected_error)));
        assert_eq!(target.base.bytes, source_bytes);
        let old = target.base.scores[0].unwrap();
        let noop = root_model::capture(&source_bytes)
            .preview_ace16_split_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(0, old, old)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(noop.candidate_bytes(), source_bytes);
    }
}

#[test]
fn original_requests_limits_and_late_cancellation_remain_atomic_and_retryable() {
    let target = source(6, false, true);
    check(&target);
    let old = target.base.scores[0].unwrap();
    for updates in [
        vec![Int32Update::new(0, old + 1, target.replacement)],
        vec![request(&target), request(&target)],
    ] {
        let result = root_model::capture(&target.base.bytes).preview_ace16_split_int32_updates(
            "Items",
            "Score",
            &updates,
            &mut budget(),
            &|| false,
        );
        assert_eq!(result.err(), Some(EditError::Conflict));
    }
    for mut limited in [
        ResourceBudget::new(0, 1_000_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 0),
    ] {
        let result = root_model::capture(&target.base.bytes).preview_ace16_split_int32_updates(
            "Items",
            "Score",
            &[request(&target)],
            &mut limited,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0);
    let complete = root_model::capture(&target.base.bytes)
        .preview_ace16_split_int32_updates(
            "Items",
            "Score",
            &[request(&target)],
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
    let total = calls.get();
    assert!(total > 2);
    calls.set(0);
    let cancelled = root_model::capture(&target.base.bytes).preview_ace16_split_int32_updates(
        "Items",
        "Score",
        &[request(&target)],
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            calls.get() >= total - 1
        },
    );
    assert_eq!(
        cancelled.err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(
        run(&target).unwrap().candidate_bytes(),
        complete.candidate_bytes()
    );
}
