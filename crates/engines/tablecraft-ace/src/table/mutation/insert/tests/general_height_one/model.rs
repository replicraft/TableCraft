//! Independent literal child-overflow sources and complete INSERT images.
use super::super::model as base;
use crate::RowInsert;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as literal;

pub(super) const PAGE: usize = 4096;

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

#[derive(Clone)]
pub(super) struct Branch {
    pub number: u32,
    pub entries: Vec<[u8; 9]>,
    pub seed: usize,
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<(i32, Option<Vec<u8>>)>,
    pub branches: Vec<Branch>,
    pub data: Vec<u32>,
    pub hints: Vec<u32>,
    pub aliases: bool,
    pub bounded: bool,
    pub promoted_maps: bool,
}

fn node(original: &[u8], branches: &[Branch], bounded: bool) -> Option<Vec<u8>> {
    let count = branches.len() - usize::from(!bounded);
    let entries = branches[..count]
        .iter()
        .map(|branch| *branch.entries.last().unwrap())
        .collect::<Vec<_>>();
    let seed = usize::from(u16::from_le_bytes(original[24..26].try_into().unwrap()));
    let (prefix, active) = if entries.is_empty() {
        (0, 0)
    } else {
        prefix(&entries, seed, true)
    };
    if active > 3616 {
        return None;
    }
    let mut page = original.to_vec();
    page[..2].copy_from_slice(&[3, 1]);
    put(&mut page, 2, (3616 - active) as u32, 2);
    put(&mut page, 4, 10, 4);
    put(&mut page, 12, 0, 4);
    put(&mut page, 16, 0, 4);
    put(
        &mut page,
        20,
        if bounded {
            0
        } else {
            branches.last().unwrap().number
        },
        4,
    );
    put(&mut page, 24, prefix as u32, 2);
    page[26] = 1;
    page[27..480].fill(0);
    let mut end = 0;
    for (position, (branch, entry)) in branches.iter().zip(entries).enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        page[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        page[480 + end..484 + end].copy_from_slice(&branch.number.to_be_bytes());
        end += 4;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    Some(page)
}

fn bitmap(members: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in members {
        assert!(number < 32736);
        page[4 + number as usize / 8] |= 1 << (number % 8);
    }
    page
}

fn indirect(number: u32) -> Vec<u8> {
    let mut row = vec![0; 69];
    row[0] = 1;
    put(&mut row, 1, number, 4);
    row
}

pub(super) fn source(
    code: u8,
    aliases: bool,
    groups: &[Vec<i32>],
    bounded: bool,
    root_seed: usize,
    late_bitmap: bool,
) -> Case {
    assert!(!groups.is_empty());
    let ids = groups.iter().flatten().copied().collect::<Vec<_>>();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    let data_start = 64_u32.max(40 + groups.len() as u32);
    let data = (0..ids.len().div_ceil(255))
        .map(|position| data_start + position as u32)
        .collect::<Vec<_>>();
    let eof = if late_bitmap {
        600
    } else {
        128.max(data.last().unwrap() + 9)
    };
    assert!(eof < 512 || late_bitmap);
    let mut before = base::source(code, aliases, false, None);
    before.resize(eof as usize * PAGE, 0);
    put(&mut before, 10 * PAGE + 16, ids.len() as u32, 4);
    put(&mut before, 10 * PAGE + 67, ids.len() as u32, 4);
    for (number, chunk) in data.iter().zip(ids.chunks(255)) {
        let rows = chunk
            .iter()
            .map(|id| (0, record(*id, None)))
            .collect::<Vec<_>>();
        let mut page = literal::data_page(10, &rows);
        put(&mut page, 2, (4082 - rows.len() * 15) as u32, 2);
        install(&mut before, *number, &page);
    }
    let mut position = 0;
    let mut branches = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let entries = group
            .iter()
            .map(|id| {
                let reference = (data[position / 255] << 8) | (position % 255) as u32;
                position += 1;
                key(*id, reference)
            })
            .collect::<Vec<_>>();
        branches.push(Branch {
            number: 32 + index as u32,
            seed: common(&entries),
            entries,
        });
    }
    for (position, branch) in branches.iter().enumerate() {
        let mut residue = vec![0; PAGE];
        residue[480..].fill(0x93 + (position % 29) as u8);
        let previous = position
            .checked_sub(1)
            .map_or(0, |index| branches[index].number);
        let next = branches.get(position + 1).map_or(0, |branch| branch.number);
        install(
            &mut before,
            branch.number,
            &leaf(Some(&residue), &branch.entries, branch.seed, previous, next),
        );
    }
    let mut root = vec![0; PAGE];
    root[480..].fill(0xda);
    put(&mut root, 24, root_seed as u32, 2);
    install(&mut before, 13, &node(&root, &branches, bounded).unwrap());
    let index_members = std::iter::once(13).chain(branches.iter().map(|branch| branch.number));
    install(
        &mut before,
        14,
        &literal::data_page(0, &[(0, inline(index_members))]),
    );
    if late_bitmap {
        install(&mut before, 597, &bitmap(data.iter().copied()));
        install(&mut before, 598, &bitmap([7]));
        install(&mut before, 599, &bitmap(eof..32736));
        install(
            &mut before,
            11,
            &literal::data_page(0, &[(0, indirect(597)), (0, inline(data.iter().copied()))]),
        );
        install(
            &mut before,
            1,
            &literal::data_page(1, &[(0, indirect(599))]),
        );
    } else {
        install(
            &mut before,
            11,
            &literal::data_page(
                0,
                &[
                    (0, inline(data.iter().copied())),
                    (0, inline(data.iter().copied())),
                ],
            ),
        );
        install(
            &mut before,
            1,
            &literal::data_page(1, &[(0, inline(eof..512))]),
        );
    }
    Case {
        before,
        rows: ids.into_iter().map(|id| (id, None)).collect(),
        branches,
        hints: data.clone(),
        data,
        aliases,
        bounded,
        promoted_maps: late_bitmap,
    }
}

pub(super) struct Expected {
    pub bytes: Vec<u8>,
    pub branches: Vec<Branch>,
    pub references: Vec<u32>,
    pub data: Vec<u32>,
    pub hints: Vec<u32>,
    pub bitmap: Option<u32>,
}

fn append_data(bytes: &mut Vec<u8>, hints: &mut Vec<u32>, owned: &mut Vec<u32>, row: &Row) -> u32 {
    let payload = record(row.id, row.binary.as_deref());
    let mut number = *hints.last().unwrap();
    let mut base = number as usize * PAGE;
    let mut count = usize::from(u16::from_le_bytes(
        bytes[base + 12..base + 14].try_into().unwrap(),
    ));
    let free = usize::from(u16::from_le_bytes(
        bytes[base + 2..base + 4].try_into().unwrap(),
    ));
    if count == 255 || payload.len() + 2 > free {
        hints.pop();
        number = (bytes.len() / PAGE) as u32;
        let mut fresh = vec![0; PAGE];
        fresh[..2].copy_from_slice(&[1, 1]);
        put(&mut fresh, 2, 4082, 2);
        put(&mut fresh, 4, 10, 4);
        bytes.extend(fresh);
        owned.push(number);
        hints.push(number);
        count = 0;
        base = number as usize * PAGE;
    }
    let end = if count == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[base + 12 + 2 * count..base + 14 + 2 * count]
                .try_into()
                .unwrap(),
        )) & 0x1fff
    };
    let start = end - payload.len();
    bytes[base + start..base + end].copy_from_slice(&payload);
    put(bytes, base + 14 + 2 * count, start as u32, 2);
    put(bytes, base + 12, (count + 1) as u32, 2);
    put(bytes, base + 2, (start - 16 - 2 * count) as u32, 2);
    (number << 8) | count as u32
}

pub(super) fn expected(case: &Case, rows: &[Row]) -> Option<Expected> {
    let mut bytes = case.before.clone();
    let mut data = case.data.clone();
    let mut hints = case.hints.clone();
    let references = rows
        .iter()
        .map(|row| append_data(&mut bytes, &mut hints, &mut data, row))
        .collect::<Vec<_>>();
    let mut groups = case.branches.clone();
    for (row, reference) in rows.iter().zip(&references) {
        let wire = key(row.id, *reference);
        let position = case.branches[..case.branches.len() - 1]
            .iter()
            .position(|branch| wire <= *branch.entries.last().unwrap())
            .unwrap_or(case.branches.len() - 1);
        groups[position].entries.push(wire);
    }
    let mut symbolic = Vec::<Branch>::new();
    let mut logical = Vec::<usize>::new();
    let mut fresh_order = Vec::<usize>::new();
    for mut branch in groups {
        branch.entries.sort_unstable();
        assert!(
            branch
                .entries
                .windows(2)
                .all(|pair| pair[0][..5] != pair[1][..5])
        );
        let original = symbolic.len();
        symbolic.push(branch);
        let mut local = vec![original];
        let mut work = vec![original];
        if prefix(&symbolic[original].entries, symbolic[original].seed, false).1 > 3616 {
            let half = symbolic[original].entries.len() / 2;
            let right = symbolic[original].entries.split_off(half);
            let left = std::mem::replace(&mut symbolic[original].entries, right);
            let added = symbolic.len();
            symbolic.push(Branch {
                number: 0,
                entries: left,
                seed: 0,
            });
            fresh_order.push(added);
            local.insert(0, added);
            work.push(added);
        }
        loop {
            let mut split = false;
            let mut cursor = 0;
            while cursor < work.len() {
                let index = work[cursor];
                if prefix(&symbolic[index].entries, symbolic[index].seed, false).1 > 3616 {
                    let half = symbolic[index].entries.len() / 2;
                    let right = symbolic[index].entries.split_off(half);
                    let left = std::mem::replace(&mut symbolic[index].entries, right);
                    let added = symbolic.len();
                    symbolic.push(Branch {
                        number: 0,
                        entries: left,
                        seed: 0,
                    });
                    fresh_order.push(added);
                    work.push(added);
                    let position = local.iter().position(|item| *item == index).unwrap();
                    local.insert(position, added);
                    split = true;
                }
                cursor += 1;
            }
            if !split {
                break;
            }
        }
        logical.extend(local);
    }
    // The first split's newly appended LEFT follows RIGHT in the worklist, so
    // both are considered before RIGHT is revisited by the next stable pass.
    let mut bitmap_page = None;
    for index in fresh_order {
        let number = (bytes.len() / PAGE) as u32;
        symbolic[index].number = number;
        bytes.extend(vec![0; PAGE]);
        if case.promoted_maps && bitmap_page.is_none() {
            let metadata = (bytes.len() / PAGE) as u32;
            bytes.extend(vec![0; PAGE]);
            bitmap_page = Some(metadata);
        }
    }
    let branches = logical
        .into_iter()
        .map(|index| symbolic[index].clone())
        .collect::<Vec<_>>();
    let root = node(&case.before[13 * PAGE..14 * PAGE], &branches, case.bounded)?;
    install(&mut bytes, 13, &root);
    for (position, branch) in branches.iter().enumerate() {
        let original = (branch.number < (case.before.len() / PAGE) as u32).then(|| {
            &case.before[branch.number as usize * PAGE..(branch.number as usize + 1) * PAGE]
        });
        let previous = position
            .checked_sub(1)
            .map_or(0, |index| branches[index].number);
        let next = branches.get(position + 1).map_or(0, |branch| branch.number);
        install(
            &mut bytes,
            branch.number,
            &leaf(original, &branch.entries, branch.seed, previous, next),
        );
    }
    let index_members = std::iter::once(13)
        .chain(branches.iter().map(|branch| branch.number))
        .collect::<Vec<_>>();
    if case.promoted_maps {
        install(&mut bytes, 597, &bitmap(data.iter().copied()));
        let eof = (bytes.len() / PAGE) as u32;
        install(&mut bytes, 599, &bitmap(eof..32736));
        install(
            &mut bytes,
            11,
            &literal::data_page(0, &[(0, indirect(597)), (0, inline(hints.iter().copied()))]),
        );
        if let Some(number) = bitmap_page {
            install(&mut bytes, number, &bitmap(index_members));
            install(
                &mut bytes,
                14,
                &literal::data_page(0, &[(0, indirect(number))]),
            );
        }
    } else {
        install(
            &mut bytes,
            11,
            &literal::data_page(
                0,
                &[
                    (0, inline(data.iter().copied())),
                    (0, inline(hints.iter().copied())),
                ],
            ),
        );
        install(
            &mut bytes,
            14,
            &literal::data_page(0, &[(0, inline(index_members))]),
        );
        let eof = (bytes.len() / PAGE) as u32;
        install(
            &mut bytes,
            1,
            &literal::data_page(1, &[(0, inline(eof..512))]),
        );
    }
    put(
        &mut bytes,
        10 * PAGE + 16,
        (case.rows.len() + rows.len()) as u32,
        4,
    );
    put(
        &mut bytes,
        10 * PAGE + 67,
        (case.rows.len() + rows.len()) as u32,
        4,
    );
    Some(Expected {
        bytes,
        branches,
        references,
        data,
        hints,
        bitmap: bitmap_page,
    })
}

pub(super) fn next(case: &Case, rows: &[Row], expected: Expected) -> Case {
    let mut all_rows = case.rows.clone();
    all_rows.extend(rows.iter().map(|row| (row.id, row.binary.clone())));
    Case {
        before: expected.bytes,
        rows: all_rows,
        branches: expected.branches,
        data: expected.data,
        hints: expected.hints,
        aliases: case.aliases,
        bounded: case.bounded,
        promoted_maps: case.promoted_maps,
    }
}
