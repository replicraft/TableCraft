use super::*;
use fixture_model::{HEIGHT, Kind, MAPS, PAGE, ROOTS, TAIL};

const INLINE_HEADER: usize = 5;
const LATE_WINDOW_PAGES: usize = 32;
const WIDE: [Kind; 2] = [Kind::Int64, Kind::Currency];

#[test]
fn repeated_root_halves_preserve_all_rows_aliases_residue_and_fresh_zero_bytes() {
    for code in [5, 6] {
        for kind in WIDE {
            let target = root(code, kind, 900, false);
            check(&target);
            let expected = expected(&target);
            let actual = run(&target).unwrap();
            assert_eq!(actual.candidate_bytes(), expected.bytes);
            assert_eq!(
                actual.candidate_bytes().len(),
                target.source.bytes.len() + 6 * PAGE
            );
            let eof = target.source.eof;
            assert_eq!(expected.logical_allocations[0], [eof + 1, eof + 2, eof]);
            assert_eq!(expected.logical_allocations[1], [eof + 5, eof + 4, eof + 3]);
            assert_eq!(expected.logical_counts[0], [450, 225, 225]);
            assert_eq!(expected.logical_counts[1], [225, 225, 450]);
            for number in ROOTS {
                assert_eq!(actual.candidate_bytes()[number * PAGE + HEIGHT], 1);
                assert_eq!(
                    root_model::entries(actual.candidate_bytes(), number).len(),
                    2
                );
            }
        }
    }
}

#[test]
fn two_offenders_visit_appended_left_in_same_pass_before_revisiting_right() {
    for kind in WIDE {
        let target = root(6, kind, 1600, true);
        check(&target);
        let expected = expected(&target);
        let actual = run(&target).unwrap();
        assert_eq!(actual.candidate_bytes(), expected.bytes);
        assert_eq!(
            actual.candidate_bytes().len(),
            target.source.bytes.len() + 12 * PAGE
        );
        for physical in 0..2 {
            let first = target.source.eof + physical * 6;
            assert_eq!(
                expected.logical_allocations[physical],
                [first + 4, first + 3, first + 1, first + 2, first + 5, first]
            );
            assert_eq!(
                expected.logical_counts[physical],
                [200, 200, 400, 400, 200, 200]
            );
        }
        let first_global = root_model::map_byte(&target.source.bytes, 1, target.source.eof);
        assert_eq!(actual.candidate_bytes()[first_global], 0);
        assert_eq!(actual.candidate_bytes()[first_global + 1], 0xf0);
        let mut reverse = requests(&target);
        reverse.reverse();
        let reversed = root_model::capture(&target.source.bytes)
            .preview_ace16_split_fixed_updates("Items", "Score", &reverse, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(reversed.candidate_bytes(), actual.candidate_bytes());
    }
}

#[test]
fn repeated_fresh_internal_parent_halves_grow_bounded_root_and_retain_original_leaves() {
    for kind in WIDE {
        let target = parent(kind);
        check(&target);
        let expected = expected(&target);
        let actual = run(&target).unwrap();
        assert_eq!(actual.candidate_bytes(), expected.bytes);
        assert_eq!(
            actual.candidate_bytes().len(),
            target.source.bytes.len() + 5 * PAGE
        );
        for (physical, count) in [(0, 2), (1, 3)] {
            let root = ROOTS[physical];
            assert_eq!(actual.candidate_bytes()[root * PAGE + HEIGHT], 2);
            assert_eq!(
                root_model::get32(actual.candidate_bytes(), root * PAGE + TAIL),
                0
            );
            assert_eq!(
                root_model::entries(actual.candidate_bytes(), root).len(),
                count
            );
            for leaf in &target.source.leaves[physical] {
                assert_eq!(actual.candidate_bytes()[leaf * PAGE], 4);
                assert_eq!(actual.candidate_bytes()[leaf * PAGE + HEIGHT], 0);
            }
        }
    }
}

#[test]
fn repeated_nonroot_leaf_splits_preserve_stored_null_peer_and_root_form() {
    for kind in WIDE {
        let target = null_peer(kind);
        check(&target);
        let expected = expected(&target);
        let actual = run(&target).unwrap();
        assert_eq!(actual.candidate_bytes(), expected.bytes);
        assert_eq!(
            actual.candidate_bytes().len(),
            target.source.bytes.len() + 4 * PAGE
        );
        assert_eq!(target.source.values.last(), Some(&None));
        let table = read_table(
            &mut Cursor::new(actual.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows().last().unwrap()[1], Value::Null);
        for root in ROOTS {
            assert_eq!(actual.candidate_bytes()[root * PAGE + HEIGHT], 1);
            assert_eq!(
                root_model::get32(actual.candidate_bytes(), root * PAGE + TAIL),
                0
            );
        }
    }
}

#[test]
fn repeated_plans_keep_exact_carrier_stale_duplicate_and_future_map_guards() {
    let target = root(6, Kind::Currency, 900, false);
    check(&target);
    for updates in [
        vec![FixedScalarUpdate::new(
            0,
            Kind::Currency.value(1),
            Kind::Currency.value(i64::MAX),
        )],
        vec![FixedScalarUpdate::new(
            0,
            Kind::Int64.value(0),
            Kind::Currency.value(i64::MAX),
        )],
        vec![FixedScalarUpdate::new(
            0,
            Kind::Currency.value(0),
            Kind::Int64.value(i64::MAX),
        )],
    ] {
        let actual = root_model::capture(&target.source.bytes)
            .preview_ace16_split_fixed_updates("Items", "Score", &updates, &mut budget(), &|| false)
            .err();
        let wrong_carrier = matches!(updates[0].expected(), Value::Int64(_))
            || matches!(updates[0].value(), Value::Int64(_));
        assert_eq!(
            actual,
            Some(if wrong_carrier {
                EditError::Unsupported(WriteBlocker::Column)
            } else {
                EditError::Conflict
            })
        );
    }
    let duplicate = vec![requests(&target).remove(0), requests(&target).remove(0)];
    assert_eq!(
        root_model::capture(&target.source.bytes)
            .preview_ace16_split_fixed_updates("Items", "Score", &duplicate, &mut budget(), &|| {
                false
            },)
            .err(),
        Some(EditError::Conflict)
    );
    for mode in 0..3 {
        let mut malformed = root(6, Kind::Currency, 900, false);
        if mode == 0 {
            fixture_model::install(
                &mut malformed.source.bytes,
                1,
                &support::data_page(1, &[(0, vec![0; 8])]),
            );
        } else if mode == 1 {
            let at = root_model::map_byte(&malformed.source.bytes, 1, malformed.source.eof);
            malformed.source.bytes[at] &= !(1 << (malformed.source.eof % 8));
        } else {
            let mut owned = vec![0; 8];
            owned[INLINE_HEADER + ROOTS[0] / 8] = 1 << (ROOTS[0] % 8);
            fixture_model::install(
                &mut malformed.source.bytes,
                MAPS[0],
                &support::data_page(0, &[(0, owned)]),
            );
        }
        check(&malformed);
        assert_eq!(
            run(&malformed).err(),
            Some(EditError::Inspect(if mode == 1 {
                InspectError::Malformed
            } else {
                InspectError::Unsupported
            }))
        );
    }
    // The initial pair at EOF30/31 is represented. The third required page32
    // isolates a late missing cell after the first split was already planned.
    let represented = late_source(Kind::Currency);
    check(&represented);
    assert_eq!(
        run(&represented).unwrap().candidate_bytes(),
        expected(&represented).bytes
    );
    for mode in 0..3 {
        let mut late = late_source(Kind::Currency);
        if mode == 0 {
            let mut global = vec![0; INLINE_HEADER + LATE_WINDOW_PAGES / 8];
            global[INLINE_HEADER + late.source.eof / 8] = 0xc0;
            fixture_model::install(
                &mut late.source.bytes,
                1,
                &support::data_page(1, &[(0, global)]),
            );
        } else if mode == 1 {
            let mut owned = vec![0; INLINE_HEADER + LATE_WINDOW_PAGES / 8];
            owned[INLINE_HEADER + ROOTS[0] / 8] = 1 << (ROOTS[0] % 8);
            fixture_model::install(
                &mut late.source.bytes,
                MAPS[0],
                &support::data_page(0, &[(0, owned)]),
            );
        } else {
            let at = root_model::map_byte(&late.source.bytes, 1, late.source.eof + 2);
            late.source.bytes[at] &= !(1 << ((late.source.eof + 2) % 8));
        }
        check(&late);
        assert_eq!(
            run(&late).err(),
            Some(EditError::Inspect(if mode == 2 {
                InspectError::Malformed
            } else {
                InspectError::Unsupported
            }))
        );
    }
    assert_eq!(
        run(&target).unwrap().candidate_bytes(),
        expected(&target).bytes
    );
}

#[test]
fn repeated_work_is_paid_and_final_cancellation_preserves_exact_retry() {
    let target = root(6, Kind::Int64, 1600, true);
    check(&target);
    for limits in [
        (1, 1_000_000, 100_000_000),
        (256 * 1024 * 1024, 1, 100_000_000),
        (256 * 1024 * 1024, 1_000_000, 1),
    ] {
        assert!(matches!(
            root_model::capture(&target.source.bytes).preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &requests(&target),
                &mut ResourceBudget::new(limits.0, limits.1, limits.2),
                &|| false,
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let measured = Cell::new(0);
    let complete = root_model::capture(&target.source.bytes)
        .preview_ace16_split_fixed_updates(
            "Items",
            "Score",
            &requests(&target),
            &mut budget(),
            &|| {
                measured.set(measured.get() + 1);
                false
            },
        )
        .unwrap();
    assert_eq!(complete.candidate_bytes(), expected(&target).bytes);
    assert!(measured.get() > 1000);
    let threshold = measured.get() - 1;
    let calls = Cell::new(0);
    assert_eq!(
        root_model::capture(&target.source.bytes)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &requests(&target),
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == threshold
                },
            )
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(calls.get(), threshold);
    assert_eq!(
        run(&target).unwrap().candidate_bytes(),
        complete.candidate_bytes()
    );
}
