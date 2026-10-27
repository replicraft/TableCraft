use super::*;
use std::cell::Cell;

#[test]
fn split_retains_root_full_bounds_links_exact_maps_and_old_residual() {
    for code in [5, 6] {
        for descending in [false, true] {
            let before = source(code, descending, true);
            assert_checked(&before);
            assert_eq!(word(&before, ROOT * PAGE + 2), 613);
            let output = preview(&before, &[Int32Update::new(0, 1, i32::MAX)]).unwrap();
            let after = output.candidate_bytes();
            assert_checked(after);
            assert_eq!(output.changed_rows(), 1);
            assert_eq!(output.before_bytes(), before);
            assert_eq!(after.len(), before.len() + 2 * PAGE);
            assert_eq!(&after[ROOT * PAGE..ROOT * PAGE + 2], &[3, 1]);
            assert_eq!(after[ROOT * PAGE + 26], 1);
            assert_eq!(number(after, ROOT * PAGE + 20), RIGHT as u32);
            assert_eq!(number(after, LEFT * PAGE + 12), 0);
            assert_eq!(number(after, LEFT * PAGE + 16), RIGHT as u32);
            assert_eq!(number(after, RIGHT * PAGE + 12), LEFT as u32);
            assert_eq!(number(after, RIGHT * PAGE + 16), 0);
            let expected = keys(descending, Some(i32::MAX));
            assert_eq!(entries(after, LEFT), expected[..250]);
            assert_eq!(entries(after, RIGHT), expected[250..]);
            let mut separator = expected[249].clone();
            separator.extend((LEFT as u32).to_be_bytes());
            assert_eq!(entries(after, ROOT), vec![separator]);
            assert_eq!(
                &after[ROOT * PAGE + 493..(ROOT + 1) * PAGE],
                &before[ROOT * PAGE + 493..(ROOT + 1) * PAGE]
            );
            assert_eq!(number(after, 10 * PAGE + 16), ROWS as u32);
            assert_eq!(number(after, 10 * PAGE + 67), HISTORY);
            let owned = bitmap_offset(&before, 14, RIGHT);
            let global = bitmap_offset(&before, 1, RIGHT);
            assert_eq!((before[owned], after[owned]), (0, 3));
            assert_eq!((before[global], after[global]), (255, 252));
            assert_eq!(&after[11 * PAGE..12 * PAGE], &before[11 * PAGE..12 * PAGE]);
            let actual = rows(after);
            assert_eq!(actual.len(), ROWS);
            assert_eq!(actual[0], vec![Value::Int32(1), Value::Int32(i32::MAX)]);
            assert_eq!(actual[1..], rows(&before)[1..]);
            assert_eq!(
                index_rows(after, "ScoreKey"),
                index_rows(after, "ScoreAlias")
            );
        }
    }
}

#[test]
fn old_service_stays_closed_while_fitted_and_noop_new_calls_keep_length() {
    let before = source(6, false, false);
    assert_eq!(
        capture(&before)
            .preview_ace16_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(0, 1, i32::MAX)],
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
    for updates in [Vec::new(), vec![Int32Update::new(0, 1, 1)]] {
        assert_eq!(
            preview(&before, &updates).unwrap().candidate_bytes(),
            before
        );
    }
    let fitted = preview(&before, &[Int32Update::new(0, 1, 0)]).unwrap();
    assert_eq!(fitted.candidate_bytes().len(), before.len());
    assert_checked(fitted.candidate_bytes());
    assert_eq!(fitted.candidate_bytes()[ROOT * PAGE], 4);
    assert_eq!(
        &fitted.candidate_bytes()[PAGE..2 * PAGE],
        &before[PAGE..2 * PAGE]
    );
    assert_eq!(
        &fitted.candidate_bytes()[14 * PAGE..15 * PAGE],
        &before[14 * PAGE..15 * PAGE]
    );
}

#[test]
fn missing_representation_and_nonfree_future_cells_refuse_without_candidate() {
    let healthy = source(6, false, false);
    for kind in 0..3 {
        let mut before = healthy.clone();
        match kind {
            0 => {
                let short = vec![0, 0, 0, 0, 0, 0, 0x20];
                before[14 * PAGE..15 * PAGE].copy_from_slice(&support::data_page(0, &[(0, short)]));
            }
            1 => {
                let offset = bitmap_offset(&before, 1, RIGHT);
                before[offset] &= !1;
            }
            _ => {
                let offset = bitmap_offset(&before, 14, RIGHT);
                before[offset] |= 1;
            }
        }
        let original = before.clone();
        assert!(preview(&before, &[Int32Update::new(0, 1, i32::MAX)]).is_err());
        assert_eq!(before, original);
    }
    assert!(preview(&healthy, &[Int32Update::new(0, 1, i32::MAX)]).is_ok());
}

#[test]
fn duplicate_stale_and_unique_conflicts_preserve_source() {
    let before = source(6, false, true);
    assert_eq!(
        preview(&before, &[Int32Update::new(0, 0, i32::MAX)]).err(),
        Some(EditError::Conflict)
    );
    assert!(
        preview(
            &before,
            &[Int32Update::new(0, 1, 2), Int32Update::new(0, 1, 3)]
        )
        .is_err()
    );
    assert_eq!(
        preview(&before, &[Int32Update::new(0, 1, 2)]).err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        ),))
    );
    assert_eq!(rows(&before)[0], vec![Value::Int32(1), Value::Int32(1)]);
}

#[test]
fn cumulative_limits_late_cancel_and_retry_are_atomic() {
    let before = source(6, false, false);
    let updates = [Int32Update::new(0, 1, i32::MAX)];
    for mut limited in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&before)
                .preview_ace16_split_int32_updates(
                    "Items",
                    "Score",
                    &updates,
                    &mut limited,
                    &|| false,
                )
                .is_err()
        );
    }
    let calls = Cell::new(0);
    let first = capture(&before)
        .preview_ace16_split_int32_updates("Items", "Score", &updates, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let stop = calls.get() - 1;
    let seen = Cell::new(0);
    assert_eq!(
        capture(&before)
            .preview_ace16_split_int32_updates("Items", "Score", &updates, &mut budget(), &|| {
                let current = seen.get();
                seen.set(current + 1);
                current >= stop
            },)
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(
        preview(&before, &updates).unwrap().candidate_bytes(),
        first.candidate_bytes()
    );
}

#[test]
fn a_fitting_followup_in_the_new_internal_tree_matches_the_existing_api() {
    let original = source(6, false, true);
    let first = preview(&original, &[Int32Update::new(0, 1, i32::MAX)]).unwrap();
    let before = first.candidate_bytes();
    let updates = [Int32Update::new(0, i32::MAX, i32::MIN)];
    let new = preview(before, &updates).unwrap();
    let old = capture(before)
        .preview_ace16_int32_updates("Items", "Score", &updates, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(new.candidate_bytes(), old.candidate_bytes());
    assert_eq!(new.candidate_bytes().len(), before.len());
    assert_eq!(
        &new.candidate_bytes()[PAGE..2 * PAGE],
        &before[PAGE..2 * PAGE]
    );
    assert_eq!(
        &new.candidate_bytes()[14 * PAGE..15 * PAGE],
        &before[14 * PAGE..15 * PAGE]
    );
    assert_checked(new.candidate_bytes());
}
