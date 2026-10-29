//! Literal two-leaf source and expected NULL-key movement across old partitions.
use super::{model::*, support};
use tablecraft_core::values::Value;

const PAGE_BYTES: usize = 4096;
const SLOT_START: usize = 14;
const MAP_HEADER: usize = 5;
const ENTRY_START: usize = 480;
const BITMAP: usize = 27;
const HEIGHT: usize = 26;
const CHILDREN: [usize; 2] = [25, 26];

fn map_start(bytes: &[u8], page: usize) -> usize {
    let at = page * PAGE_BYTES + SLOT_START;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    page * PAGE_BYTES + usize::from(word & 0x1fff) + MAP_HEADER
}

fn entries(bytes: &[u8], page: usize) -> Vec<Vec<u8>> {
    let at = page * PAGE_BYTES;
    let mut entries = Vec::new();
    let mut start = 0;
    for end in 1..=PAGE_BYTES - ENTRY_START {
        if bytes[at + BITMAP + end / 8] & (1 << (end % 8)) != 0 {
            entries.push(bytes[at + ENTRY_START + start..at + ENTRY_START + end].to_vec());
            start = end;
        }
    }
    entries
}

fn repack(bytes: &mut [u8], page: usize, entries: &[Vec<u8>]) {
    let at = page * PAGE_BYTES;
    bytes[at + BITMAP..at + ENTRY_START].fill(0);
    let mut end = 0;
    for entry in entries {
        let start = at + ENTRY_START + end;
        bytes[start..start + entry.len()].copy_from_slice(entry);
        end += entry.len();
        bytes[at + BITMAP + end / 8] |= 1 << (end % 8);
    }
    support::put16(bytes, at + 2, (PAGE_BYTES - ENTRY_START - end) as u16);
}

fn source_tree(code: u8) -> Model {
    let mut model = source(code, false, true);
    let old = entries(&model.bytes, root(0));
    assert_eq!(old.len(), 4);
    model.bytes.resize(27 * PAGE_BYTES, 0);
    let mut bounds = Vec::new();
    for (position, number) in CHILDREN.into_iter().enumerate() {
        let at = number * PAGE_BYTES;
        model.bytes[at..at + 2].copy_from_slice(&[4, 1]);
        support::put32(&mut model.bytes, at + 4, definition(0) as u32);
        support::put32(
            &mut model.bytes,
            at + 12,
            if position == 0 { 0 } else { 25 },
        );
        support::put32(
            &mut model.bytes,
            at + 16,
            if position == 0 { 26 } else { 0 },
        );
        let selected = &old[position * 2..position * 2 + 2];
        repack(&mut model.bytes, number, selected);
        let mut bound = selected.last().unwrap().clone();
        bound.extend((number as u32).to_be_bytes());
        bounds.push(bound);
        let owned = map_start(&model.bytes, definition(0) + 3);
        model.bytes[owned + number / 8] |= 1 << (number % 8);
        let global = map_start(&model.bytes, 1);
        model.bytes[global + number / 8] &= !(1 << (number % 8));
    }
    let at = root(0) * PAGE_BYTES;
    model.bytes[at] = 3;
    model.bytes[at + HEIGHT] = 1;
    repack(&mut model.bytes, root(0), &bounds);
    model
}

fn wanted(model: &Model, delete_child: bool) -> Expected {
    let deleted: &[usize] = if delete_child { &[0] } else { &[] };
    let nulled: &[usize] = if delete_child { &[1] } else { &[0, 1] };
    let mut result = expected(model, [deleted, &[0], &[]], nulled);
    // Restore original node/leaf scratch before independent structural replay.
    for number in [root(0), CHILDREN[0], CHILDREN[1]] {
        result.bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
            .copy_from_slice(&model.bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]);
    }
    let mut keys = Vec::new();
    for (row, values) in model.rows[0].iter().enumerate() {
        if deleted.contains(&row) {
            continue;
        }
        assert!(nulled.contains(&row) || values[1] == Value::Null);
        let mut entry = vec![0];
        entry.extend((((data(0) as u32) << 8) | row as u32).to_be_bytes());
        keys.push(entry);
    }
    keys.sort();
    // The first original leaf had two surviving NULL rows. The second had
    // the two now-NULL children, less any explicitly removed child.
    let mut bounds = Vec::new();
    let mut consumed = 0;
    for (position, number) in CHILDREN.into_iter().enumerate() {
        let count = if position == 0 {
            2
        } else {
            keys.len() - consumed
        };
        let selected = &keys[consumed..consumed + count];
        repack(&mut result.bytes, number, selected);
        let mut bound = selected.last().unwrap().clone();
        bound.extend((number as u32).to_be_bytes());
        bounds.push(bound);
        consumed += count;
    }
    assert_eq!(consumed, keys.len());
    repack(&mut result.bytes, root(0), &bounds);
    result
}

#[test]
fn literal_two_leaf_sources_and_expected_images_qualify_before_the_api() {
    for code in [5, 6] {
        let model = source_tree(code);
        before(&model);
        qualify(&wanted(&model, false));
        qualify(&wanted(&model, true));
    }
}

#[test]
fn null_keys_move_between_original_leaves_and_update_both_parent_bounds() {
    for code in [5, 6] {
        let model = source_tree(code);
        for delete_child in [false, true] {
            let expected = wanted(&model, delete_child);
            qualify(&expected);
            let candidate = if delete_child {
                run(&model, &[(1, &[0]), (0, &[0])]).unwrap()
            } else {
                run(&model, &[(1, &[0])]).unwrap()
            };
            verify(&model, &expected, &candidate);
        }
    }
}
