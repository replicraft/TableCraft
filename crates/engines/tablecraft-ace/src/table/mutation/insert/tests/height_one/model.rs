//! Literal next-INSERT images derived from the existing independent split image.
use super::super::split::model as split;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;

pub(super) fn source(code: u8, aliases: bool) -> (Vec<u8>, Vec<Vec<Value>>) {
    let original = split::source(code, aliases, false);
    let (bytes, _) = split::expected(&original);
    let mut rows = original.rows;
    rows.push(vec![Value::Int32(226), Value::Binary(Vec::new())]);
    (bytes, rows)
}

fn entries(page: &[u8]) -> Vec<(i32, u32)> {
    let active = 3616 - u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
    let prefix = u16::from_le_bytes(page[24..26].try_into().unwrap()) as usize;
    let mut previous = 0;
    let mut first = Vec::new();
    let mut result = Vec::new();
    for end in 1..=active {
        if page[27 + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let mut wire = if previous == 0 {
            Vec::new()
        } else {
            first[..prefix].to_vec()
        };
        wire.extend_from_slice(&page[480 + previous..480 + end]);
        assert_eq!(wire.len(), 9);
        assert_eq!(wire[0], 127);
        if previous == 0 {
            first = wire.clone();
        }
        result.push((
            (u32::from_be_bytes(wire[1..5].try_into().unwrap()) ^ 0x80000000) as i32,
            u32::from_be_bytes(wire[5..9].try_into().unwrap()),
        ));
        previous = end;
    }
    assert_eq!(previous, active);
    result
}

pub(super) fn minimum_left_bytes(before: &[u8], additions: &[i32]) -> usize {
    let mut values = entries(&before[17 * PAGE..18 * PAGE])
        .into_iter()
        .map(|(value, _)| value)
        .collect::<Vec<_>>();
    values.extend_from_slice(additions);
    values.sort();
    let keys = values
        .iter()
        .map(|value| {
            [
                vec![127],
                ((*value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    let mut common = 5;
    for key in &keys[1..] {
        common = keys[0][..common]
            .iter()
            .zip(key)
            .take_while(|(left, right)| left == right)
            .count();
    }
    keys.len() * 9 - common * (keys.len() - 1)
}

fn leaf(original: &[u8], keys: &[(i32, u32)]) -> Vec<u8> {
    let wires = keys
        .iter()
        .map(|(value, row)| {
            [
                vec![127],
                ((*value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                row.to_be_bytes().to_vec(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    let mut common = 5;
    for wire in &wires[1..] {
        common = wires[0][..common]
            .iter()
            .zip(wire)
            .take_while(|(a, b)| a == b)
            .count();
    }
    let seed = u16::from_le_bytes(original[24..26].try_into().unwrap()) as usize;
    let mut prefix = seed.min(common);
    if wires.len() * 9 - prefix * (wires.len() - 1) > 3616 {
        prefix = common;
    }
    let mut bytes = original.to_vec();
    bytes[27..480].fill(0);
    support::put16(&mut bytes, 24, prefix as u16);
    let mut end = 0;
    for (position, wire) in wires.iter().enumerate() {
        let suffix = &wire[if position == 0 { 0 } else { prefix }..];
        bytes[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert!(end <= 3616);
    support::put16(&mut bytes, 2, (3616 - end) as u16);
    bytes
}

pub(super) fn expected(before: &[u8], values: &[i32]) -> Vec<u8> {
    let mut after = before.to_vec();
    let mut left = entries(&before[17 * PAGE..18 * PAGE]);
    let mut right = entries(&before[16 * PAGE..17 * PAGE]);
    assert_eq!(left.len(), 226);
    assert_eq!(right.len(), 226);
    assert_eq!(left.last().unwrap().0, -1);
    for value in values {
        let data = &mut after[15 * PAGE..16 * PAGE];
        let count = u16::from_le_bytes(data[12..14].try_into().unwrap()) as usize;
        assert!((197..255).contains(&count));
        let last_slot = 14 + (count - 1) * 2;
        let end = u16::from_le_bytes(data[last_slot..last_slot + 2].try_into().unwrap()) as usize;
        let mut row = vec![2, 0];
        row.extend(value.to_le_bytes());
        row.extend([6, 0, 6, 0, 1, 0, 3]);
        assert_eq!(row.len(), 13);
        let start = end - row.len();
        data[start..end].copy_from_slice(&row);
        support::put16(data, 14 + count * 2, start as u16);
        support::put16(data, 12, (count + 1) as u16);
        support::put16(data, 2, (start - 14 - 2 * (count + 1)) as u16);
        let pair = (*value, (15 << 8) | count as u32);
        if *value <= -1 {
            left.push(pair);
        } else {
            right.push(pair);
        }
    }
    left.sort();
    right.sort();
    for (page, keys) in [(17, left), (16, right)] {
        let start = page * PAGE;
        after[start..start + PAGE].copy_from_slice(&leaf(&before[start..start + PAGE], &keys));
    }
    for offset in [10 * PAGE + 16, 10 * PAGE + 67] {
        assert_eq!(
            u32::from_le_bytes(before[offset..offset + 4].try_into().unwrap()),
            452
        );
        support::put32(&mut after, offset, 452 + values.len() as u32);
    }
    after
}

pub(super) fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    if let Some(offset) = actual.iter().zip(expected).position(|(a, b)| a != b) {
        panic!(
            "whole-image mismatch at {offset} (page {}, offset {}): actual {:02x}, expected {:02x}",
            offset / PAGE,
            offset % PAGE,
            actual[offset],
            expected[offset]
        );
    }
}
