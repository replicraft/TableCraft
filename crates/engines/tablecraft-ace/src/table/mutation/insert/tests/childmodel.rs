//! Independent literal child-overflow sources and complete INSERT images.
use super::super::model as base;
use crate::RowInsert;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as literal;

pub(super) const PAGE: usize = 4096;

#[derive(Clone, Copy)]
pub(super) enum Side {
    Left,
    Right,
}

pub(super) struct Row {
    pub id: i32,
    pub binary: Option<Vec<u8>>,
}

impl Row {
    pub fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            self.binary
                .as_ref()
                .map_or(Value::Null, |bytes| Value::Binary(bytes.clone())),
        ]
    }

    pub fn request(&self) -> RowInsert {
        RowInsert::new(self.values())
    }
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub ids: Vec<i32>,
    pub aliases: bool,
}

fn put(bytes: &mut [u8], at: usize, value: u32, width: usize) {
    bytes[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
}

fn install(bytes: &mut [u8], number: u32, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    let start = number as usize * PAGE;
    bytes[start..start + PAGE].copy_from_slice(page);
}

fn inline(members: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for number in members {
        assert!(number < 512);
        bytes[5 + number as usize / 8] |= 1 << (number % 8);
    }
    bytes
}

fn record(id: i32, binary: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(id.to_le_bytes());
    if let Some(payload) = binary {
        bytes.extend_from_slice(payload);
    }
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend([6, 0, 1, 0, if binary.is_some() { 3 } else { 1 }]);
    bytes
}

fn key(id: i32, reference: u32) -> [u8; 9] {
    let mut bytes = [0; 9];
    bytes[0] = 127;
    bytes[1..5].copy_from_slice(&((id as u32) ^ 0x80000000).to_be_bytes());
    bytes[5..].copy_from_slice(&reference.to_be_bytes());
    bytes
}

fn common(entries: &[[u8; 9]]) -> usize {
    let mut width = 9;
    for entry in &entries[1..] {
        width = entries[0][..width]
            .iter()
            .zip(entry)
            .take_while(|(left, right)| left == right)
            .count();
    }
    width
}

fn prefix(entries: &[[u8; 9]], seed: usize, child: bool) -> (usize, usize) {
    assert!(!entries.is_empty());
    let best = common(entries);
    let mut prefix = seed.min(best);
    let full = entries.len() * (9 + if child { 4 } else { 0 });
    let saved = entries.len() - 1;
    if full - saved * prefix > 3616 {
        prefix = best;
    }
    (prefix, full - saved * prefix)
}

fn leaf(
    original: Option<&[u8]>,
    entries: &[[u8; 9]],
    seed: usize,
    previous: u32,
    next: u32,
) -> Vec<u8> {
    let (prefix, active) = prefix(entries, seed, false);
    assert!(active <= 3616);
    let mut page = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    page[..2].copy_from_slice(&[4, 1]);
    put(&mut page, 2, (3616 - active) as u32, 2);
    put(&mut page, 4, 10, 4);
    put(&mut page, 12, previous, 4);
    put(&mut page, 16, next, 4);
    put(&mut page, 20, 0, 4);
    put(&mut page, 24, prefix as u32, 2);
    page[26] = 0;
    page[27..480].fill(0);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let wire = &entry[if position == 0 { 0 } else { prefix }..];
        page[480 + end..480 + end + wire.len()].copy_from_slice(wire);
        end += wire.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    if let Some(original) = original {
        assert_eq!(&page[480 + active..], &original[480 + active..]);
    } else {
        assert!(page[480 + active..].iter().all(|byte| *byte == 0));
    }
    page
}

fn root(original: &[u8], leaves: &[(u32, Vec<[u8; 9]>)]) -> Vec<u8> {
    let entries = leaves[..leaves.len() - 1]
        .iter()
        .map(|(_, entries)| *entries.last().unwrap())
        .collect::<Vec<_>>();
    let seed = usize::from(u16::from_le_bytes(original[24..26].try_into().unwrap()));
    let (prefix, active) = prefix(&entries, seed, true);
    assert!(active <= 3616);
    let mut page = original.to_vec();
    page[..2].copy_from_slice(&[3, 1]);
    put(&mut page, 2, (3616 - active) as u32, 2);
    put(&mut page, 4, 10, 4);
    put(&mut page, 12, 0, 4);
    put(&mut page, 16, 0, 4);
    put(&mut page, 20, leaves.last().unwrap().0, 4);
    put(&mut page, 24, prefix as u32, 2);
    page[26] = 1;
    page[27..480].fill(0);
    let mut end = 0;
    for (position, ((child, _), entry)) in leaves.iter().zip(&entries).enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        page[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        page[480 + end..480 + end + 4].copy_from_slice(&child.to_be_bytes());
        end += 4;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    assert_eq!(&page[480 + active..], &original[480 + active..]);
    page
}

pub(super) fn source(code: u8, aliases: bool, side: Side, data_full: bool) -> Case {
    let ids = match side {
        Side::Left => std::iter::once(i32::MIN)
            .chain(-450..0)
            .chain([1])
            .collect::<Vec<_>>(),
        Side::Right => std::iter::once(-1)
            .chain(1..451)
            .chain([i32::MAX])
            .collect::<Vec<_>>(),
    };
    assert_eq!(ids.len(), 452);
    let mut before = base::source(code, aliases, false, None);
    before.resize(20 * PAGE, 0);
    put(&mut before, 10 * PAGE + 16, 452, 4);
    put(&mut before, 10 * PAGE + 67, 452, 4);
    for (number, chunk) in [(12, &ids[..255]), (15, &ids[255..])] {
        let mut rows = chunk
            .iter()
            .map(|id| (0, record(*id, None)))
            .collect::<Vec<_>>();
        if number == 15 && data_full {
            rows.push((0x8000, vec![0x9d; 1118]));
        }
        let used = rows.iter().map(|(_, row)| row.len()).sum::<usize>();
        let free = PAGE - 14 - 2 * rows.len() - used;
        let mut page = literal::data_page(10, &rows);
        put(&mut page, 2, free as u32, 2);
        install(&mut before, number, &page);
    }
    install(
        &mut before,
        11,
        &literal::data_page(0, &[(0, inline([12, 15])), (0, inline([12, 15]))]),
    );
    install(
        &mut before,
        14,
        &literal::data_page(0, &[(0, inline([13, 17, 18]))]),
    );
    install(
        &mut before,
        1,
        &literal::data_page(1, &[(0, inline(20..512))]),
    );
    let entries = ids
        .iter()
        .enumerate()
        .map(|(position, id)| {
            let (page, slot) = if position < 255 {
                (12, position)
            } else {
                (15, position - 255)
            };
            key(*id, (page << 8) | slot as u32)
        })
        .collect::<Vec<_>>();
    let split = match side {
        Side::Left => 451,
        Side::Right => 1,
    };
    let mut left = vec![0; PAGE];
    left[480..].fill(0xa7);
    let mut right = vec![0; PAGE];
    right[480..].fill(0xb9);
    install(
        &mut before,
        17,
        &leaf(
            Some(&left),
            &entries[..split],
            if split == 451 { 1 } else { 0 },
            0,
            18,
        ),
    );
    install(
        &mut before,
        18,
        &leaf(
            Some(&right),
            &entries[split..],
            if split == 1 { 1 } else { 0 },
            17,
            0,
        ),
    );
    let mut original_root = vec![0; PAGE];
    original_root[480..].fill(0xd3);
    install(
        &mut before,
        13,
        &root(
            &original_root,
            &[
                (17, entries[..split].to_vec()),
                (18, entries[split..].to_vec()),
            ],
        ),
    );
    let target = match side {
        Side::Left => 17,
        Side::Right => 18,
    };
    assert_eq!(
        u16::from_le_bytes(
            before[target * PAGE + 2..target * PAGE + 4]
                .try_into()
                .unwrap()
        ),
        7
    );
    Case {
        before,
        ids,
        aliases,
    }
}

pub(super) fn inserted(side: Side) -> i32 {
    match side {
        Side::Left => -451,
        Side::Right => 451,
    }
}

pub(super) fn peer(side: Side) -> i32 {
    match side {
        Side::Left => 2,
        Side::Right => -2,
    }
}

pub(super) fn expected(case: &Case, rows: &[Row]) -> (Vec<u8>, Vec<u32>) {
    let mut bytes = case.before.clone();
    let mut hints = vec![12_u32, 15];
    let mut owned = vec![12_u32, 15];
    let mut refs = Vec::new();
    for row in rows {
        let payload = record(row.id, row.binary.as_deref());
        let mut page = *hints.last().unwrap();
        let mut count = u16::from_le_bytes(
            bytes[page as usize * PAGE + 12..page as usize * PAGE + 14]
                .try_into()
                .unwrap(),
        ) as usize;
        let free = u16::from_le_bytes(
            bytes[page as usize * PAGE + 2..page as usize * PAGE + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        if count == 255 || payload.len() + 2 > free {
            hints.pop();
            page = (bytes.len() / PAGE) as u32;
            let mut fresh = vec![0; PAGE];
            fresh[..2].copy_from_slice(&[1, 1]);
            put(&mut fresh, 2, 4082, 2);
            put(&mut fresh, 4, 10, 4);
            bytes.extend(fresh);
            owned.push(page);
            hints.push(page);
            count = 0;
        }
        let base = page as usize * PAGE;
        let end = if count == 0 {
            PAGE
        } else {
            u16::from_le_bytes(
                bytes[base + 14 + 2 * (count - 1)..base + 16 + 2 * (count - 1)]
                    .try_into()
                    .unwrap(),
            ) as usize
                & 0x1fff
        };
        let start = end - payload.len();
        assert!(start >= 14 + 2 * (count + 1));
        bytes[base + start..base + end].copy_from_slice(&payload);
        put(&mut bytes, base + 14 + 2 * count, start as u32, 2);
        put(&mut bytes, base + 12, (count + 1) as u32, 2);
        put(
            &mut bytes,
            base + 2,
            (start - (14 + 2 * (count + 1))) as u32,
            2,
        );
        refs.push((page << 8) | count as u32);
    }
    let mut all = case
        .ids
        .iter()
        .enumerate()
        .map(|(position, id)| {
            let (page, slot) = if position < 255 {
                (12, position)
            } else {
                (15, position - 255)
            };
            (*id, key(*id, (page << 8) | slot as u32))
        })
        .collect::<Vec<_>>();
    all.extend(
        rows.iter()
            .zip(&refs)
            .map(|(row, reference)| (row.id, key(row.id, *reference))),
    );
    all.sort_unstable_by_key(|entry| entry.0);
    assert!(all.windows(2).all(|pair| pair[0].0 != pair[1].0));
    let groups = [
        all.iter()
            .filter(|entry| entry.0 <= -1)
            .map(|entry| entry.1)
            .collect::<Vec<_>>(),
        all.iter()
            .filter(|entry| entry.0 > -1)
            .map(|entry| entry.1)
            .collect::<Vec<_>>(),
    ];
    let mut leaves = Vec::new();
    let mut added = Vec::new();
    for (position, entries) in groups.into_iter().enumerate() {
        let original = 17 + position as u32;
        let seed = u16::from_le_bytes(
            case.before[original as usize * PAGE + 24..original as usize * PAGE + 26]
                .try_into()
                .unwrap(),
        ) as usize;
        if prefix(&entries, seed, false).1 > 3616 {
            let half = entries.len() / 2;
            let new_left = (bytes.len() / PAGE) as u32;
            bytes.extend(vec![0; PAGE]);
            added.push(new_left);
            leaves.push((new_left, entries[..half].to_vec()));
            leaves.push((original, entries[half..].to_vec()));
        } else {
            leaves.push((original, entries));
        }
    }
    assert_eq!(added.len(), 1);
    for (position, (number, entries)) in leaves.iter().enumerate() {
        let previous = if position == 0 {
            0
        } else {
            leaves[position - 1].0
        };
        let next = leaves.get(position + 1).map_or(0, |entry| entry.0);
        let old = if *number < 20 {
            Some(&case.before[*number as usize * PAGE..(*number as usize + 1) * PAGE])
        } else {
            None
        };
        let seed = old.map_or(0, |page| {
            u16::from_le_bytes(page[24..26].try_into().unwrap()) as usize
        });
        install(
            &mut bytes,
            *number,
            &leaf(old, entries, seed, previous, next),
        );
    }
    install(
        &mut bytes,
        13,
        &root(&case.before[13 * PAGE..14 * PAGE], &leaves),
    );
    install(
        &mut bytes,
        11,
        &literal::data_page(0, &[(0, inline(owned.iter().copied())), (0, inline(hints))]),
    );
    let mut indexes = vec![13, 17, 18];
    indexes.extend(added);
    install(
        &mut bytes,
        14,
        &literal::data_page(0, &[(0, inline(indexes))]),
    );
    let eof = (bytes.len() / PAGE) as u32;
    install(
        &mut bytes,
        1,
        &literal::data_page(1, &[(0, inline(eof..512))]),
    );
    put(&mut bytes, 10 * PAGE + 16, (452 + rows.len()) as u32, 4);
    put(&mut bytes, 10 * PAGE + 67, (452 + rows.len()) as u32, 4);
    (bytes, refs)
}
