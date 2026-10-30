//! Literal nullable populations and complete UPDATE images, without planner codecs.
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::multiple_indexes as frames;

pub const PAGE: usize = 4096;
const ROWS: usize = 500;
const LEFT_HISTORY: u32 = 700;
const PAIR_HISTORY: u32 = 800;

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Mixed,
    Empty,
    Branch,
    Clear,
    BranchClear,
}

#[derive(Clone)]
pub struct Row {
    pub id: i32,
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub reference: u32,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub original: Vec<Row>,
    pub final_rows: Vec<Row>,
    pub added: Vec<(u32, usize)>,
    pub source_eof: u32,
    pub final_history: [u32; 2],
    pub target: Option<u32>,
}

fn signed(position: usize) -> i32 {
    if position.is_multiple_of(2) {
        i32::MIN + position as i32 * 17
    } else {
        i32::MAX - position as i32 * 17
    }
}

fn record(row: &Row) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.left.map_or([0xa5; 4], i32::to_le_bytes));
    bytes.extend(row.right.map_or([0x5a; 4], i32::to_le_bytes));
    bytes.push(1 | (u8::from(row.left.is_some()) << 1) | (u8::from(row.right.is_some()) << 2));
    assert_eq!(bytes.len(), 15);
    bytes
}

fn component(value: Option<i32>, descending: bool) -> Vec<u8> {
    let mut bytes = value.map_or_else(
        || vec![0],
        |value| {
            let mut bytes = vec![127];
            bytes.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
            bytes
        },
    );
    if descending {
        for byte in &mut bytes {
            *byte = !*byte;
        }
    }
    bytes
}

fn entries(rows: &[Row], physical: usize, descending: bool) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .filter_map(|row| {
            if physical == 1 && row.left.is_none() && row.right.is_none() {
                return None;
            }
            let mut key = if physical == 0 {
                component(row.left, descending)
            } else {
                let mut key = component(row.right, !descending);
                key.extend(component(row.left, descending));
                key
            };
            let key_length = key.len();
            key.extend(row.reference.to_be_bytes());
            Some((key, key_length, row.reference))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| (&left.0[..left.1], left.2).cmp(&(&right.0[..right.1], right.2)));
    entries.into_iter().map(|entry| entry.0).collect()
}

fn fits(entries: &[Vec<u8>]) -> bool {
    if entries.is_empty() {
        return true;
    }
    let common = entries
        .iter()
        .skip(1)
        .fold(entries[0].len(), |common, entry| {
            entries[0][..common]
                .iter()
                .zip(entry)
                .take_while(|(left, right)| left == right)
                .count()
        });
    let total = entries.iter().map(Vec::len).sum::<usize>();
    total - common * (entries.len() - 1) <= 3616
}

fn node(original: Option<&[u8]>, children: &[(u32, Vec<u8>)]) -> Vec<u8> {
    assert!(children.len() >= 2);
    let mut page = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    page[..480].fill(0);
    page[..2].copy_from_slice(&[3, 1]);
    raw::put32(&mut page, 4, 10);
    raw::put32(&mut page, 20, children.last().unwrap().0);
    page[26] = 1;
    let mut end = 0;
    for (child, maximum) in &children[..children.len() - 1] {
        let start = 480 + end;
        page[start..start + maximum.len()].copy_from_slice(maximum);
        page[start + maximum.len()..start + maximum.len() + 4]
            .copy_from_slice(&child.to_be_bytes());
        end += maximum.len() + 4;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    raw::put16(&mut page, 2, (3616 - end) as u16);
    page
}

fn data_page(rows: &[Row], flags: u16) -> Vec<u8> {
    let records = rows
        .iter()
        .map(|row| (flags, record(row)))
        .collect::<Vec<_>>();
    let mut page = raw::data_page(10, &records);
    raw::put16(&mut page, 2, (PAGE - 14 - records.len() * 17) as u16);
    page
}

pub(super) fn span(bytes: &[u8], reference: u32) -> std::ops::Range<usize> {
    let base = (reference >> 8) as usize * PAGE;
    let slot = (reference & 255) as usize;
    let start = usize::from(u16::from_le_bytes(
        bytes[base + 14 + slot * 2..base + 16 + slot * 2]
            .try_into()
            .unwrap(),
    )) & 0x1fff;
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[base + 12 + slot * 2..base + 14 + slot * 2]
                .try_into()
                .unwrap(),
        )) & 0x1fff
    };
    base + start..base + end
}

fn definition(bytes: &mut [u8], descending: bool, unique: bool) {
    let base = 10 * PAGE;
    raw::put32(bytes, base + 16, ROWS as u32);
    raw::put16(bytes, base + 43, 0);
    bytes[base + 137] = 4;
    bytes[base + 152] = 3;
    bytes[base + 153] = 0;
    raw::put16(bytes, base + 158, 8);
    raw::put16(bytes, base + 160, 4);
    raw::put32(bytes, base + 67, LEFT_HISTORY);
    raw::put32(bytes, base + 79, PAIR_HISTORY);
    raw::put16(bytes, base + 196, 1);
    bytes[base + 198] = u8::from(!descending);
    bytes[base + 238] = 0x80 | u8::from(unique);
    raw::put16(bytes, base + 248, 2);
    bytes[base + 250] = u8::from(descending);
    raw::put16(bytes, base + 251, 1);
    bytes[base + 253] = u8::from(!descending);
    bytes[base + 290] = 0x82 | u8::from(unique);
    let names = ["LeftOrder", "LeftAlias", "PairOrder", "PairAlias"];
    bytes[base + 408..base + PAGE].fill(0);
    let mut position = base + 408;
    for (logical, name) in names.iter().enumerate() {
        bytes[base + 296 + logical * 28 + 23] = 0;
        let name = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(bytes, position, name.len() as u16);
        position += 2;
        bytes[position..position + name.len()].copy_from_slice(&name);
        position += name.len();
    }
    raw::put16(bytes, position, 0xffff);
}

/// Construct source and complete expectations before invoking any write API.
pub fn source(code: u8, shape: Shape, descending: bool, unique: bool, forwarded: bool) -> Case {
    let branch = matches!(shape, Shape::Branch | Shape::BranchClear);
    let clearing = matches!(shape, Shape::Clear | Shape::BranchClear);
    let empty = matches!(shape, Shape::Empty);
    let mut before = frames::source(code, false, false).before;
    before.resize(if branch { 22 } else { 18 } * PAGE, 0);
    definition(&mut before, descending, unique);
    let mut original = Vec::new();
    for position in 0..ROWS {
        let page = [12_u32, 16, 17][position / 200];
        original.push(Row {
            id: position as i32 + 1,
            left: (!empty && position >= 300).then(|| signed(position)),
            right: None,
            reference: (page << 8) | (position % 200) as u32,
        });
    }
    for (page, rows) in [12, 16, 17].into_iter().zip(original.chunks(200)) {
        frames::install(&mut before, page, &data_page(rows, 0));
    }
    let mut owners = [vec![13], vec![14]];
    for (physical, owned) in owners.iter_mut().enumerate() {
        let keys = entries(&original, physical, descending);
        if branch {
            let middle = keys.len() / 2;
            let left = 18 + physical as u32 * 2;
            let right = left + 1;
            let halves = [&keys[..middle], &keys[middle..]];
            for (position, half) in halves.into_iter().enumerate() {
                frames::install(
                    &mut before,
                    left + position as u32,
                    &frames::leaf(
                        None,
                        half,
                        0,
                        if position == 0 { 0 } else { left },
                        if position == 0 { right } else { 0 },
                    ),
                );
            }
            frames::install(
                &mut before,
                13 + physical as u32,
                &node(
                    None,
                    &[
                        (left, keys[middle - 1].clone()),
                        (right, keys.last().unwrap().clone()),
                    ],
                ),
            );
            owned.extend([left, right]);
        } else {
            assert!(fits(&keys));
            let mut leaf = frames::leaf(None, &keys, 0, 0, 0);
            let active = 3616 - usize::from(u16::from_le_bytes(leaf[2..4].try_into().unwrap()));
            leaf[480 + active..].fill(0x90 + physical as u8);
            frames::install(&mut before, 13 + physical as u32, &leaf);
        }
    }
    let mut data = vec![12, 16, 17];
    let target = if forwarded {
        let target = (before.len() / PAGE) as u32;
        before.extend(data_page(&original[..1], 0x8000));
        let root = span(&before, original[0].reference);
        before[root.clone()].fill(0);
        raw::put32(&mut before, root.start, target << 8);
        let directory = 12 * PAGE + 14;
        let word = u16::from_le_bytes(before[directory..directory + 2].try_into().unwrap());
        raw::put16(&mut before, directory, word | 0x4000);
        data.push(target);
        Some(target << 8)
    } else {
        None
    };
    frames::map_page(
        &mut before,
        11,
        vec![frames::map(data), frames::map([12, 16, 17])],
    );
    frames::map_page(
        &mut before,
        15,
        owners.iter().cloned().map(frames::map).collect(),
    );
    let source_eof = (before.len() / PAGE) as u32;
    frames::map_page(&mut before, 1, vec![frames::map(source_eof..512)]);

    let mut final_rows = original.clone();
    for (position, row) in final_rows.iter_mut().enumerate() {
        if clearing && position >= 300 {
            row.left = None;
            row.right = None;
        } else {
            row.left = Some(signed(position));
            row.right = Some(signed(position + ROWS));
        }
    }
    let mut expected = before.clone();
    for (position, row) in final_rows.iter().enumerate() {
        let reference = if position == 0 {
            target.unwrap_or(row.reference)
        } else {
            row.reference
        };
        let range = span(&before, reference);
        let start = range.start;
        if let Some(value) = row.left {
            expected[start + 6..start + 10].copy_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = row.right {
            expected[start + 10..start + 14].copy_from_slice(&value.to_le_bytes());
        }
        expected[range.end - 1] =
            1 | (u8::from(row.left.is_some()) << 1) | (u8::from(row.right.is_some()) << 2);
    }
    let mut added = Vec::new();
    let mut next = source_eof;
    for (physical, owned) in owners.iter_mut().enumerate() {
        let old = entries(&original, physical, descending);
        let keys = entries(&final_rows, physical, descending);
        if branch {
            let first = old.len() / 2;
            let left = 18 + physical as u32 * 2;
            let right = left + 1;
            let mut leaves = vec![(left, keys[..first].to_vec())];
            if fits(&keys[first..]) {
                leaves.push((right, keys[first..].to_vec()));
            } else {
                let middle = first + (keys.len() - first) / 2;
                let fresh = next;
                next += 1;
                added.push((fresh, physical));
                owned.push(fresh);
                leaves.push((fresh, keys[first..middle].to_vec()));
                leaves.push((right, keys[middle..].to_vec()));
            }
            for (position, (page, entries)) in leaves.iter().enumerate() {
                let original = if *page < source_eof {
                    Some(&before[*page as usize * PAGE..(*page as usize + 1) * PAGE])
                } else {
                    None
                };
                let previous = if position == 0 {
                    0
                } else {
                    leaves[position - 1].0
                };
                let following = leaves.get(position + 1).map_or(0, |leaf| leaf.0);
                if *page >= source_eof {
                    expected.resize((*page as usize + 1) * PAGE, 0);
                }
                frames::install(
                    &mut expected,
                    *page,
                    &frames::leaf(original, entries, 0, previous, following),
                );
            }
            let children = leaves
                .iter()
                .map(|(page, keys)| (*page, keys.last().unwrap().clone()))
                .collect::<Vec<_>>();
            let root = 13 + physical as u32;
            frames::install(
                &mut expected,
                root,
                &node(
                    Some(&before[root as usize * PAGE..(root as usize + 1) * PAGE]),
                    &children,
                ),
            );
        } else {
            assert!(
                !fits(&keys),
                "The final keys must still overflow after full prefix repair"
            );
            let right = next;
            let left = next + 1;
            next += 2;
            added.extend([(right, physical), (left, physical)]);
            owned.extend([right, left]);
            expected.resize(next as usize * PAGE, 0);
            let middle = keys.len() / 2;
            frames::install(
                &mut expected,
                left,
                &frames::leaf(None, &keys[..middle], 0, 0, right),
            );
            frames::install(
                &mut expected,
                right,
                &frames::leaf(None, &keys[middle..], 0, left, 0),
            );
            let root = 13 + physical as u32;
            frames::install(
                &mut expected,
                root,
                &node(
                    Some(&before[root as usize * PAGE..(root as usize + 1) * PAGE]),
                    &[
                        (left, keys[middle - 1].clone()),
                        (right, keys.last().unwrap().clone()),
                    ],
                ),
            );
        }
    }
    let introduced = original
        .iter()
        .zip(&final_rows)
        .filter(|(old, new)| {
            old.left.is_none() && old.right.is_none() && (new.left.is_some() || new.right.is_some())
        })
        .count();
    let final_history = [LEFT_HISTORY, PAIR_HISTORY + introduced as u32];
    raw::put32(&mut expected, 10 * PAGE + 79, final_history[1]);
    frames::map_page(
        &mut expected,
        15,
        owners.into_iter().map(frames::map).collect(),
    );
    frames::map_page(&mut expected, 1, vec![frames::map(next..512)]);
    Case {
        before,
        expected,
        original,
        final_rows,
        added,
        source_eof,
        final_history,
        target,
    }
}
