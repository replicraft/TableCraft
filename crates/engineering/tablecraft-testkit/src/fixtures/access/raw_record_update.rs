//! Independent stored-field replacement, preserving sibling raw spans and padding.
use super::{put16, raw_forwarding_chain, raw_record_placement as placement};

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Independently replace one physically present field while keeping its stored
/// count and the bytes of every sibling and original padding span unchanged.
pub(in crate::fixtures::access) fn text(
    before: &[u8],
    data_page: u32,
    root: usize,
    field: usize,
    column: usize,
    value: Option<&str>,
) -> Vec<u8> {
    let old = placement::record(before, data_page, root);
    let count = word(old, 0);
    assert!(column < count);
    assert_eq!(before[field], 10);
    let variable = word(before, field + 7);
    let mask = old.len() - count.div_ceil(8);
    if value.is_none() && old[mask + column / 8] & (1 << (column % 8)) == 0 {
        return before.to_vec();
    }
    let variables = word(old, mask - 2);
    assert!(variable < variables);
    let data_end = mask - 2 - 2 * (variables + 1);
    let offsets = (0..=variables)
        .map(|index| word(old, mask - 4 - 2 * index))
        .collect::<Vec<_>>();
    let start = offsets[variable];
    let stop = offsets[variable + 1];
    let end = offsets[variables];
    let mut payload = Vec::new();
    if let Some(value) = value {
        let units = value.encode_utf16().collect::<Vec<_>>();
        assert!(units.len() <= word(before, field + 23) / 2);
        if before[field + 16] & 1 != 0
            && units.len() > 2
            && units.iter().all(|unit| (1..=255).contains(unit))
        {
            payload.extend([255, 254]);
            payload.extend(units.iter().map(|unit| *unit as u8));
        } else {
            assert!(!value.starts_with('\u{feff}'));
            for unit in units {
                payload.extend(unit.to_le_bytes());
            }
        }
    }
    let mut row = old[..start].to_vec();
    row.extend(&payload);
    row.extend_from_slice(&old[stop..end]);
    row.extend_from_slice(&old[end..data_end]);
    let new_data_end = row.len().max(data_end);
    row.resize(new_data_end, 0);
    for (index, offset) in offsets.into_iter().enumerate().rev() {
        let adjusted = if index <= variable {
            offset
        } else {
            offset - (stop - start) + payload.len()
        };
        row.extend((adjusted as u16).to_le_bytes());
    }
    row.extend((variables as u16).to_le_bytes());
    let new_mask = row.len();
    row.extend_from_slice(&old[mask..]);
    row[new_mask + column / 8] &= !(1 << (column % 8));
    if value.is_some() {
        row[new_mask + column / 8] |= 1 << (column % 8);
    }
    placement::place(before, data_page, root, &row)
}

/// NULL preserves its ignored fixed payload; supplied values replace only LONG
/// bytes and the selected native presence bit. No generator/index changes occur.
pub(in crate::fixtures::access) fn number(
    before: &[u8],
    data_page: u32,
    root: usize,
    field: usize,
    column: usize,
    value: Option<i32>,
) -> Vec<u8> {
    let old = placement::record(before, data_page, root);
    let count = word(old, 0);
    assert!(column < count);
    assert_eq!(before[field], 4);
    let start = 2 + word(before, field + 21);
    let mask = old.len() - count.div_ceil(8);
    let mut row = old.to_vec();
    if let Some(value) = value {
        row[start..start + 4].copy_from_slice(&value.to_le_bytes());
        row[mask + column / 8] |= 1 << (column % 8);
    } else {
        row[mask + column / 8] &= !(1 << (column % 8));
    }
    placement::place(before, data_page, root, &row)
}

/// Materialize the known third Currency after a complete two-field fixed prefix.
/// Callers separately validate their active declarations; this literal row owner
/// preserves old payload/mask bits and ignores no variable or opaque row spans.
pub(in crate::fixtures::access) fn historical_currency(
    before: &[u8],
    data_page: u32,
    root: usize,
    field: usize,
    value: Option<i64>,
) -> Vec<u8> {
    assert_eq!(before[field], 5);
    assert_eq!(word(before, field + 5), 2);
    assert_eq!(&before[field + 15..field + 17], &[3, 0]);
    assert_eq!(word(before, field + 21), 8);
    assert_eq!(word(before, field + 23), 8);
    let old = raw_forwarding_chain::record(before, data_page, root);
    let count = word(old, 0);
    assert!(matches!(count, 2 | 3));
    assert_eq!(old.len(), if count == 2 { 11 } else { 19 });
    let mask = old[old.len() - 1];
    let present = count == 3 && mask & 4 != 0;
    if !present && value.is_none() {
        return before.to_vec();
    }
    if present && value.is_some_and(|value| old[10..18] == value.to_le_bytes()) {
        return before.to_vec();
    }
    let mut row = old[..old.len() - 1].to_vec();
    if count == 2 {
        row.resize(18, 0);
        put16(&mut row, 0, 3);
    }
    if let Some(value) = value {
        row[10..18].copy_from_slice(&value.to_le_bytes());
    }
    row.push((mask & !4) | if value.is_some() { 4 } else { 0 });
    raw_forwarding_chain::place_terminal(before, data_page, root, &row)
}
