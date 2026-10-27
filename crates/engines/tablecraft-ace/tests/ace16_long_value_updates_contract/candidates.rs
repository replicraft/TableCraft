use super::*;

const SECOND_MEMO: usize = 15;
const SELECTED_DESCRIPTOR_OFFSET: usize = 6;

fn member(bytes: &mut [u8], page_number: usize, slot: usize, number: usize, set: bool) {
    let start = usize::from(word(bytes, page_number, slot) & OFFSET_MASK);
    let offset = page_number * PAGE + start + MAP_HEADER + number / 8;
    let mask = 1 << (number % 8);
    if set {
        bytes[offset] |= mask;
    } else {
        bytes[offset] &= !mask;
    }
}

fn expected_descriptor(bytes: &mut [u8], destination: usize, slot: usize) {
    let root = ROOT * PAGE + usize::from(word(bytes, ROOT, 0) & OFFSET_MASK);
    let start = root + SELECTED_DESCRIPTOR_OFFSET;
    bytes[start..start + 12].copy_from_slice(&single(66, destination, slot));
}

fn two_candidates(code: u8, second_rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = source(code, 1);
    set_maps(
        &mut bytes,
        &[MEMO, SECOND_MEMO],
        &[MEMO, SECOND_MEMO],
        &[OLE],
    );
    install(&mut bytes, SECOND_MEMO, &lval(second_rows));
    let checked = check(&bytes);
    assert_eq!(checked.columns()[0].candidate_pages(), 2);
    assert_eq!(read(&bytes)[0][1], Value::Text("A".into()));
    bytes
}

#[test]
fn multiple_lval_candidates_choose_highest_then_reuse_lower_after_failed_fit() {
    for code in [5, 6] {
        for full in [false, true] {
            let second_rows = if full {
                vec![(0, vec![0xa7; 4078])]
            } else {
                Vec::new()
            };
            let before = two_candidates(code, &second_rows);
            let value = Value::Text("Ā".repeat(33));
            let result = run(&before, "Memo", &[memo_update(value.clone())]).unwrap();
            let after = result.candidate_bytes();
            let mut expected = before.clone();
            let payload = [0, 1].repeat(33);
            if full {
                install(
                    &mut expected,
                    MEMO,
                    &lval(&[(0, vec![65, 0]), (0, vec![0xf1, 0xe2, 0xd3]), (0, payload)]),
                );
                member(&mut expected, 11, 3, SECOND_MEMO, false);
                expected_descriptor(&mut expected, MEMO, 2);
            } else {
                install(&mut expected, SECOND_MEMO, &lval(&[(0, payload)]));
                expected_descriptor(&mut expected, SECOND_MEMO, 0);
            }
            assert_eq!(after, expected);
            assert_eq!(read(after)[0][1], value);
            old_chunks_unchanged(&before, after);
            check(after);
        }
    }
}

#[test]
fn all_lval_candidates_are_exhausted_before_one_represented_eof_allocation() {
    for code in [5, 6] {
        let mut before = two_candidates(code, &[(0, vec![0xa7; 4078])]);
        install(
            &mut before,
            MEMO,
            &lval(&[
                (0, vec![65, 0]),
                (0, vec![0xf1, 0xe2, 0xd3]),
                (0, vec![0xb6; 4069]),
            ]),
        );
        check(&before);
        let value = Value::Text("Ā".repeat(33));
        let result = run(&before, "Memo", &[memo_update(value.clone())]).unwrap();
        let after = result.candidate_bytes();
        let mut expected = before.clone();
        expected.extend(lval(&[(0, [0, 1].repeat(33))]));
        member(&mut expected, 11, 3, SECOND_MEMO, false);
        member(&mut expected, 11, 3, MEMO, false);
        member(&mut expected, 11, 2, EOF, true);
        member(&mut expected, 11, 3, EOF, true);
        member(&mut expected, 1, 0, EOF, false);
        expected_descriptor(&mut expected, EOF, 0);
        assert_eq!(after, expected);
        assert_eq!(read(after)[0][1], value);
        old_chunks_unchanged(&before, after);
        check(after);
    }
}

#[test]
fn full_slot_directory_skips_high_candidate_without_reclaiming_its_chunks() {
    let full_directory = vec![(0, vec![0x7c]); 255];
    let before = two_candidates(6, &full_directory);
    let value = Value::Text("Ā".repeat(33));
    let result = run(&before, "Memo", &[memo_update(value.clone())]).unwrap();
    let after = result.candidate_bytes();
    let mut expected = before.clone();
    install(
        &mut expected,
        MEMO,
        &lval(&[
            (0, vec![65, 0]),
            (0, vec![0xf1, 0xe2, 0xd3]),
            (0, [0, 1].repeat(33)),
        ]),
    );
    member(&mut expected, 11, 3, SECOND_MEMO, false);
    expected_descriptor(&mut expected, MEMO, 2);
    assert_eq!(after, expected);
    assert_eq!(read(after)[0][1], value);
    assert_eq!(
        &after[SECOND_MEMO * PAGE..(SECOND_MEMO + 1) * PAGE],
        &before[SECOND_MEMO * PAGE..(SECOND_MEMO + 1) * PAGE]
    );
    check(after);
}

#[test]
fn ordinary_parent_reuses_lower_hint_without_allocating_or_consuming_other_hints() {
    let mut before = source(6, 1);
    let memo = single(2, MEMO, 0);
    let ole = single(1, OLE, 0);
    install(
        &mut before,
        ROOT,
        &page(
            10,
            &[
                (0, record(1, Some(&memo), Some(&ole))),
                (HIDDEN, vec![0x9b; 4025]),
            ],
        ),
    );
    install(&mut before, 15, &page(10, &[]));
    install(&mut before, 16, &page(10, &[(HIDDEN, vec![0xa7; 4078])]));
    install(
        &mut before,
        11,
        &page(
            0,
            &[
                (0, map(&[ROOT, 15, 16])),
                (0, map(&[ROOT, 15, 16])),
                (0, map(&[MEMO])),
                (0, map(&[MEMO])),
                (0, map(&[OLE])),
                (0, map(&[OLE])),
            ],
        ),
    );
    check(&before);
    let value = Value::Text("Ā".repeat(32));
    let result = run(&before, "Memo", &[memo_update(value.clone())]).unwrap();
    let after = result.candidate_bytes();
    let mut expected = before.clone();
    let descriptor = inline(&[0, 1].repeat(32));
    let target = record(1, Some(&descriptor), Some(&ole));
    install(&mut expected, 15, &page(10, &[(HIDDEN, target)]));
    member(&mut expected, 11, 1, 16, false);
    let start = ROOT * PAGE + usize::from(word(&before, ROOT, 0) & OFFSET_MASK);
    let length = body(&before, ROOT, 0).len();
    expected[start..start + length].fill(0);
    expected[start..start + 4].copy_from_slice(&(15_u32 << 8).to_le_bytes());
    support::put16(
        &mut expected,
        ROOT * PAGE + SLOT,
        word(&before, ROOT, 0) | FORWARDED,
    );
    assert_eq!(after, expected);
    assert_eq!(read(after)[0][1], value);
    old_chunks_unchanged(&before, after);
    check(after);
}

#[test]
fn multiple_hints_allow_noops_but_never_authorize_unowned_pages() {
    let before = two_candidates(6, &[]);
    let noop = run(&before, "Memo", &[]).unwrap();
    assert_eq!(noop.candidate_bytes(), before);
    let same = run(&before, "Memo", &[memo_update(Value::Text("A".into()))]).unwrap();
    assert_eq!(same.candidate_bytes(), before);

    let mut unowned = before.clone();
    member(&mut unowned, 11, 2, SECOND_MEMO, false);
    assert_eq!(read(&unowned)[0][1], Value::Text("A".into()));
    assert!(run(&unowned, "Memo", &[]).is_err());
    assert!(run(&unowned, "Memo", &[memo_update(Value::Null)]).is_err());
}
