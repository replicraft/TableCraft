use super::*;
use std::cell::Cell;

#[test]
fn shared_hidden_and_visible_targets_are_readable_but_not_exclusively_mutable() {
    for hidden in [false, true] {
        let target = ((DATA_PAGE as u32) << 8) | if hidden { 2 } else { 1 };
        let mut pointer = vec![0x5a; record(b"A").len()];
        pointer[..4].copy_from_slice(&target.to_le_bytes());
        let rows = if hidden {
            vec![
                (FORWARDED, pointer.clone()),
                (FORWARDED, pointer),
                (HIDDEN, record(b"A")),
            ]
        } else {
            vec![(FORWARDED, pointer), (0, record(b"A"))]
        };
        let before = image(&rows, 2, true);
        assert_eq!(
            read(&before),
            vec![vec![Value::Int32(1), Value::Binary(vec![b'A'])]; 2]
        );
        assert_eq!(
            preview(&before, b"A", b"ABCDE").err().unwrap(),
            EditError::Unsupported(WriteBlocker::RowLayout)
        );
        assert_eq!(read(&before).len(), 2);
    }
}

#[test]
fn native_255_slot_limit_allows_last_slot_then_refuses_append() {
    for original_count in [254, 255] {
        let mut rows = vec![(0, record(b"A"))];
        rows.extend((1..original_count).map(|_| (HIDDEN, vec![0xa5])));
        let before = image(&rows, 1, true);
        assert_eq!(read(&before).len(), 1);
        let result = preview(&before, b"A", b"ABCDE");
        if original_count == 254 {
            let output = result.unwrap();
            let after = output.candidate_bytes();
            assert_eq!(count(after), 255);
            assert_eq!(word(after, 0) & FORWARDED, FORWARDED);
            assert_eq!(word(after, 254) & HIDDEN, HIDDEN);
            assert_eq!(read(after)[0][1], Value::Binary(b"ABCDE".to_vec()));
            for slot in 1..original_count {
                assert_eq!(word(after, slot), word(&before, slot));
            }
        } else {
            assert_eq!(
                result.err().unwrap(),
                EditError::Unsupported(WriteBlocker::RowLayout)
            );
        }
    }
}

#[test]
fn false_hint_blocks_only_new_append_and_preserves_fitted_history() {
    let ordinary = image(&[(0, record(b"A"))], 1, false);
    read(&ordinary);
    assert_eq!(
        preview(&ordinary, b"A", b"ABCDE").err().unwrap(),
        EditError::Unsupported(WriteBlocker::RowLayout)
    );
    let admitted = image(&[(0, record(b"A"))], 1, true);
    let forwarded = preview(&admitted, b"A", b"ABCDE").unwrap();
    let mut history = forwarded.candidate_bytes().to_vec();
    // The second map record is the independent table free-space hint.
    let map_end = MAP_PAGE * PAGE_BYTES + PAGE_BYTES;
    let second_start = u16::from_le_bytes(
        history[MAP_PAGE * PAGE_BYTES + SLOT_OFFSET + SLOT_BYTES
            ..MAP_PAGE * PAGE_BYTES + SLOT_OFFSET + 2 * SLOT_BYTES]
            .try_into()
            .unwrap(),
    ) as usize;
    let hint = MAP_PAGE * PAGE_BYTES + second_start + 6;
    assert!(hint < map_end);
    history[hint] = 0;
    read(&history);
    for replacement in [b"ABCDE".as_slice(), b"B".as_slice()] {
        let output = preview(&history, b"ABCDE", replacement).unwrap();
        assert_eq!(count(output.candidate_bytes()), count(&history));
        assert_eq!(output.candidate_bytes()[hint], 0);
        assert_eq!(
            read(output.candidate_bytes())[0][1],
            Value::Binary(replacement.to_vec())
        );
        if replacement == b"ABCDE" {
            assert_eq!(output.candidate_bytes(), history);
        }
    }

    // A known empty inline window covers pages 0 through 7 only. Data12 is
    // owned and globally allocated, but there is no candidate hint cell for it.
    // Fitted updates do not need that unused cell; a new append still does.
    for (mut source, expected) in [
        (ordinary.clone(), b"A".as_slice()),
        (history.clone(), b"ABCDE".as_slice()),
    ] {
        let owned = vec![0, 0, 0, 0, 0, 0, 0x10];
        let empty_window = vec![0, 0, 0, 0, 0, 0];
        let map = support::data_page(0, &[(0, owned), (0, empty_window)]);
        let map_range = MAP_PAGE * PAGE_BYTES..(MAP_PAGE + 1) * PAGE_BYTES;
        source[map_range.clone()].copy_from_slice(&map);
        assert_eq!(read(&source).len(), 1);

        for replacement in [expected, b"B".as_slice()] {
            let output = preview(&source, expected, replacement).unwrap();
            assert_eq!(&output.candidate_bytes()[map_range.clone()], &map);
            assert_eq!(count(output.candidate_bytes()), count(&source));
            assert_eq!(
                read(output.candidate_bytes())[0][1],
                Value::Binary(replacement.to_vec())
            );
            if replacement == expected {
                assert_eq!(output.candidate_bytes(), source);
            }
        }

        assert_eq!(
            preview(&source, expected, b"ABCDEFGHIJKLMNOPQRST")
                .err()
                .unwrap(),
            EditError::Unsupported(WriteBlocker::RowLayout)
        );
        assert_eq!(&source[map_range], &map);
    }
}

#[test]
fn exact_remaining_bytes_include_new_slot_word() {
    let needed = record(b"ABCDE").len() + SLOT_BYTES;
    for available in [needed, needed - 1] {
        let filler_length =
            PAGE_BYTES - SLOT_OFFSET - 2 * SLOT_BYTES - record(b"A").len() - available;
        let rows = vec![(0, record(b"A")), (HIDDEN, vec![0xa7; filler_length])];
        let before = image(&rows, 1, true);
        read(&before);
        let result = preview(&before, b"A", b"ABCDE");
        if available == needed {
            let output = result.unwrap();
            assert_eq!(count(output.candidate_bytes()), 3);
            let start = DATA_PAGE * PAGE_BYTES + (word(&before, 1) & PHYSICAL_OFFSET) as usize;
            let end = DATA_PAGE * PAGE_BYTES + (word(&before, 0) & PHYSICAL_OFFSET) as usize;
            assert_eq!(&output.candidate_bytes()[start..end], &before[start..end]);
            assert_eq!(
                read(output.candidate_bytes())[0][1],
                Value::Binary(b"ABCDE".to_vec())
            );
        } else {
            assert_eq!(
                result.err().unwrap(),
                EditError::Unsupported(WriteBlocker::RowLayout)
            );
        }
    }
}

#[test]
fn invalid_pointer_owner_and_global_state_never_publish_candidate() {
    let healthy = image(&[(0, record(b"A"))], 1, true);
    for mutation in 0..3 {
        let mut before = healthy.clone();
        match mutation {
            0 => {
                // A root shorter than the physical four-byte reference can be
                // readable only as a corrupt pointer, never an append target.
                let rows = vec![(FORWARDED, vec![0; 3])];
                before[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
                    .copy_from_slice(&page(&rows));
            }
            1 => {
                support::put32(&mut before, DATA_PAGE * PAGE_BYTES + OWNER_OFFSET, 99);
            }
            _ => {
                // Mark the existing data page globally free; no ordinary
                // read decision is used to infer mutation ownership.
                let global_start = u16::from_le_bytes(
                    before[PAGE_BYTES + SLOT_OFFSET..PAGE_BYTES + SLOT_OFFSET + SLOT_BYTES]
                        .try_into()
                        .unwrap(),
                ) as usize;
                before[PAGE_BYTES + global_start + 5 + DATA_PAGE / 8] |= 1 << (DATA_PAGE % 8);
            }
        }
        let source = before.clone();
        assert!(preview(&before, b"A", b"ABCDE").is_err());
        assert_eq!(before, source);
    }
    assert!(preview(&healthy, b"A", b"ABCDE").is_ok());
}

#[test]
fn budget_and_cancellation_failures_preserve_source_and_retry_identity() {
    let before = image(&[(0, record(b"A"))], 1, true);
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Binary(b"A".to_vec()),
        Value::Binary(b"ABCDE".to_vec()),
    )];
    let calls = Cell::new(0);
    let cancelled = || {
        let count = calls.get();
        calls.set(count + 1);
        count >= 2
    };
    assert!(matches!(
        capture(&before).preview_ace16_page_updates(
            "Items",
            "Score",
            &updates,
            &mut budget(),
            &cancelled
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let mut exhausted = ResourceBudget::new(0, 100_000, 30_000_000);
    assert!(
        capture(&before)
            .preview_ace16_page_updates("Items", "Score", &updates, &mut exhausted, &|| false)
            .is_err()
    );
    let first = preview(&before, b"A", b"ABCDE").unwrap();
    let second = preview(&before, b"A", b"ABCDE").unwrap();
    assert_eq!(first.before_bytes(), before);
    assert_eq!(first.candidate_bytes(), second.candidate_bytes());
}
