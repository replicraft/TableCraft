use super::*;
use std::cell::Cell;

#[test]
fn two_allocations_merge_same_byte_bits_and_stop_at_representation_boundary() {
    for selected in [16, 31] {
        let mut rows = vec![(0, record(b"A")); selected];
        let filler = PAGE - 14 - 2 * (rows.len() + 1) - rows.len() * record(b"A").len();
        rows.push((HIDDEN, vec![0x96; filler]));
        let before = page_model::image(&rows, selected as u32, true);
        assert_eq!(read(&before).len(), selected);
        let updates = (0..selected)
            .map(|row| {
                SlotScalarUpdate::new(
                    row,
                    Value::Binary(b"A".to_vec()),
                    Value::Binary(vec![0x39; 255]),
                )
            })
            .collect::<Vec<_>>();
        let result = capture(&before).preview_ace16_variable_updates(
            "Items",
            "Score",
            &updates,
            &mut budget(),
            &|| false,
        );
        if selected == 16 {
            let output = result.unwrap();
            let after = output.candidate_bytes();
            assert_eq!(after.len(), 16 * PAGE);
            assert_eq!(map_byte(after, 11, 0, 1), 0xd0);
            assert_eq!(map_byte(after, 11, 1, 1), 0x80);
            assert_eq!(map_byte(after, 1, 0, 1), 0);
            assert_eq!(read(after).len(), 16);
            assert!(
                read(after)
                    .iter()
                    .all(|row| row[1] == Value::Binary(vec![0x39; 255]))
            );
        } else {
            // Thirty-one records need three pages; owned/hint maps cover
            // only through15. Never grow a map to represent page16.
            assert!(result.is_err());
        }
    }
}

#[test]
fn eof_allocation_preserves_old_pages_and_composes_existing_bitmap_byte() {
    let before = source(false, false);
    assert_eq!(read(&before)[0][1], Value::Binary(b"A".to_vec()));
    let replacement = vec![0x5b; 255];
    let output = update(&before, &replacement).unwrap();
    let after = output.candidate_bytes();
    assert_eq!(after.len(), before.len() + PAGE);
    assert_eq!(pointer(after), (EOF as u32) << 8);
    assert_eq!(word(after, ROOT, 0) & FORWARDED, FORWARDED);
    assert_eq!(word(after, EOF, 0) & HIDDEN, HIDDEN);
    assert_eq!(map_byte(after, 11, 0, 1), 0x50);
    assert_eq!(map_byte(after, 11, 1, 1), 0x40);
    assert_eq!(map_byte(&before, 1, 0, 1), 0xc0);
    assert_eq!(map_byte(after, 1, 0, 1), 0x80);
    assert_eq!(
        &after[ALTERNATE * PAGE..EOF * PAGE],
        &before[ALTERNATE * PAGE..EOF * PAGE]
    );
    assert_eq!(read(after)[0][1], Value::Binary(replacement.clone()));
    assert_eq!(&after[EOF * PAGE..EOF * PAGE + 2], &[1, 1]);
    assert_eq!(
        &after[EOF * PAGE + 4..EOF * PAGE + 8],
        &10_u32.to_le_bytes()
    );
    assert_eq!(
        &after[EOF * PAGE + 12..EOF * PAGE + 14],
        &1_u16.to_le_bytes()
    );
    let start = usize::from(word(after, EOF, 0) & OFFSET);
    assert_eq!(&after[EOF * PAGE + start..], record(&replacement));
    assert_eq!(output.before_bytes(), before);
}

#[test]
fn alternate_destination_keeps_root_reference_and_allocation_maps() {
    let before = source(true, true);
    read(&before);
    let output = update(&before, &[0x35; 255]).unwrap();
    let after = output.candidate_bytes();
    assert_eq!(after.len(), before.len());
    assert_eq!(pointer(after), ((ALTERNATE as u32) << 8) | 1);
    assert_eq!(word(after, ALTERNATE, 1) & HIDDEN, HIDDEN);
    assert_eq!(&after[PAGE..2 * PAGE], &before[PAGE..2 * PAGE]);
    assert_eq!(&after[11 * PAGE..12 * PAGE], &before[11 * PAGE..12 * PAGE]);
    assert_eq!(read(after)[0][1], Value::Binary(vec![0x35; 255]));
}

#[test]
fn highest_hint_failure_allocates_eof_without_searching_lower_fitting_page() {
    let mut before = source(true, false);
    // Root12 has ample free space but the highest hinted page13 does not.
    // Selection is highest-candidate-once, rather than first-fit searching.
    before[ROOT * PAGE..(ROOT + 1) * PAGE].copy_from_slice(&page_model::page(&[(0, record(b"A"))]));
    read(&before);
    let output = update(&before, &[0x42; 255]).unwrap();
    let after = output.candidate_bytes();
    assert_eq!(pointer(after), (EOF as u32) << 8);
    assert_eq!(after.len(), before.len() + PAGE);
    assert_eq!(map_byte(after, 11, 1, 1), 0x50);
    assert_eq!(
        &after[ALTERNATE * PAGE..EOF * PAGE],
        &before[ALTERNATE * PAGE..EOF * PAGE]
    );
}

#[test]
fn old_page_api_still_refuses_cross_page_and_eof_growth() {
    for before in [source(false, false), source(true, true)] {
        assert_eq!(
            page_model::preview(&before, b"A", &[0x21; 255])
                .err()
                .unwrap(),
            EditError::Unsupported(WriteBlocker::RowLayout)
        );
        assert!(update(&before, &[0x21; 255]).is_ok());
    }
}

#[test]
fn unrepresented_or_nonfree_future_page_never_produces_candidate() {
    let healthy = source(false, false);
    for mutation in 0..3 {
        let mut before = healthy.clone();
        match mutation {
            0 => {
                // Global EOF14 is represented but already allocated.
                let start = usize::from(word(&before, 1, 0) & OFFSET);
                before[PAGE + start + 6] &= !0x40;
            }
            1 => {
                let owned = vec![0, 0, 0, 0, 0, 0, 0x10];
                let missing = vec![0, 0, 0, 0, 0, 0];
                before[11 * PAGE..12 * PAGE]
                    .copy_from_slice(&support::data_page(0, &[(0, owned), (0, missing)]));
            }
            _ => {
                // Ordinary root remains readable; aliasing distinct role
                // references cannot supply an independent destination hint.
                support::put32(&mut before, 10 * PAGE + 59, 11 << 8);
            }
        }
        read(&before);
        let unchanged = before.clone();
        assert!(update(&before, &[0x73; 255]).is_err());
        assert_eq!(before, unchanged);
    }
    assert!(update(&healthy, &[0x73; 255]).is_ok());
}

#[test]
fn quota_and_late_cancellation_refuse_atomically_and_retry_identically() {
    let before = source(false, false);
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Binary(b"A".to_vec()),
        Value::Binary(vec![0x63; 255]),
    )];
    let expected = update(&before, &[0x63; 255]).unwrap();
    for mut limited in [
        ResourceBudget::new(0, 100_000, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&before)
                .preview_ace16_variable_updates("Items", "Score", &updates, &mut limited, &|| false)
                .is_err()
        );
    }
    let checks = Cell::new(0);
    capture(&before)
        .preview_ace16_variable_updates("Items", "Score", &updates, &mut budget(), &|| {
            checks.set(checks.get() + 1);
            false
        })
        .unwrap();
    let stop = checks.get() - 1;
    let seen = Cell::new(0);
    assert!(matches!(
        capture(&before).preview_ace16_variable_updates(
            "Items",
            "Score",
            &updates,
            &mut budget(),
            &|| {
                let current = seen.get();
                seen.set(current + 1);
                current >= stop
            }
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(
        update(&before, &[0x63; 255]).unwrap().candidate_bytes(),
        expected.candidate_bytes()
    );
}
