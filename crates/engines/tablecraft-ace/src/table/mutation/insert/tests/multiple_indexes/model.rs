//! Literal records, complete key/ROW streams and image effects for two owners.
use super::super::model as original;
use crate::test_support::base;
use std::collections::BTreeSet;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;

pub const PAGE: usize = 4096;

#[derive(Clone)]
pub struct Row {
    pub id: i32,
    pub group: Option<i32>,
    pub binary: Option<Vec<u8>>,
}
impl Row {
    pub fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            self.group.map_or(Value::Null, Value::Int32),
            self.binary
                .as_ref()
                .map_or(Value::Null, |value| Value::Binary(value.clone())),
        ]
    }
}

pub struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<(Row, u32)>,
    pub data: Vec<u32>,
    pub histories: [u32; 2],
    pub descending: bool,
}

fn record(row: &Row) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.group.unwrap_or(0).to_le_bytes());
    if let Some(binary) = &row.binary {
        bytes.extend(binary);
    }
    bytes.extend((bytes.len() as u16).to_le_bytes());
    bytes.extend([10, 0, 1, 0]);
    bytes.push(1 | (u8::from(row.group.is_some()) << 1) | (u8::from(row.binary.is_some()) << 2));
    bytes
}

pub(in crate::table::mutation::insert::tests) fn install(
    image: &mut [u8],
    number: u32,
    page: &[u8],
) {
    image[number as usize * PAGE..(number as usize + 1) * PAGE].copy_from_slice(page);
}
pub(in crate::table::mutation::insert::tests) fn map(
    members: impl IntoIterator<Item = u32>,
) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for number in members {
        assert!(number < 512);
        bytes[5 + number as usize / 8] |= 1 << (number % 8);
    }
    bytes
}
pub(in crate::table::mutation::insert::tests) fn map_page(
    image: &mut [u8],
    number: u32,
    rows: Vec<Vec<u8>>,
) {
    install(
        image,
        number,
        &raw::data_page(
            if number == 1 { 1 } else { 0 },
            &rows.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
        ),
    );
}
fn wires(rows: &[(Row, u32)], secondary: bool, descending: bool) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .map(|(row, reference)| {
            let key = if secondary { row.group } else { Some(row.id) };
            let mut wire = if let Some(key) = key {
                let mut wire = vec![127];
                wire.extend(((key as u32) ^ 0x80000000).to_be_bytes());
                wire
            } else {
                vec![0]
            };
            if secondary && descending {
                if wire[0] == 0 {
                    wire[0] = 255;
                } else {
                    for byte in &mut wire {
                        *byte = !*byte;
                    }
                }
            }
            wire.extend(reference.to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}
pub(in crate::table::mutation::insert::tests) fn framing(
    entries: &[Vec<u8>],
    seed: usize,
) -> (usize, usize) {
    if entries.is_empty() {
        return (0, 0);
    }
    let mut common = entries[0].len();
    for entry in &entries[1..] {
        common = entries[0][..common]
            .iter()
            .zip(entry)
            .take_while(|(left, right)| left == right)
            .count();
    }
    let full = entries.iter().map(Vec::len).sum::<usize>();
    let saved = entries.len() - 1;
    let mut prefix = common.min(seed);
    if full - saved * prefix > 3616 {
        prefix = common;
    }
    (prefix, full - saved * prefix)
}
pub(in crate::table::mutation::insert::tests) fn leaf(
    original: Option<&[u8]>,
    entries: &[Vec<u8>],
    seed: usize,
    previous: u32,
    next: u32,
) -> Vec<u8> {
    let (prefix, active) = framing(entries, seed);
    assert!(active <= 3616);
    let mut page = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    raw::put16(&mut page, 2, (3616 - active) as u16);
    raw::put32(&mut page, 4, 10);
    raw::put32(&mut page, 12, previous);
    raw::put32(&mut page, 16, next);
    raw::put16(&mut page, 24, prefix as u16);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        page[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    page
}
pub(in crate::table::mutation::insert::tests) fn root(
    original: &[u8],
    left: u32,
    right: u32,
    bound: &[u8],
) -> Vec<u8> {
    let mut page = original.to_vec();
    page[..480].fill(0);
    page[..2].copy_from_slice(&[3, 1]);
    let active = bound.len() + 4;
    raw::put16(&mut page, 2, (3616 - active) as u16);
    raw::put32(&mut page, 4, 10);
    raw::put32(&mut page, 20, right);
    page[26] = 1;
    page[480..480 + bound.len()].copy_from_slice(bound);
    page[480 + bound.len()..480 + active].copy_from_slice(&left.to_be_bytes());
    page[27 + active / 8] |= 1 << (active % 8);
    page
}

pub fn source(code: u8, large: bool, unique: bool) -> Case {
    let mut bytes = original::source(code, false, false, None);
    bytes.resize(if large { 18 } else { 16 } * PAGE, 0);
    let records = if large {
        (-200..0)
            .chain(1..201)
            .enumerate()
            .map(|(position, id)| Row {
                id,
                group: (position != 0).then_some(id),
                binary: Some(vec![]),
            })
            .collect::<Vec<_>>()
    } else {
        vec![
            Row {
                id: 1,
                group: Some(10),
                binary: None,
            },
            Row {
                id: 2,
                group: Some(if unique { 20 } else { 10 }),
                binary: Some(vec![]),
            },
            Row {
                id: 3,
                group: None,
                binary: Some(vec![0x80, 0, 0xff]),
            },
        ]
    };
    let histories = if large { [417, 829] } else { [17, 29] };
    let mut definition = base::definition(
        &[("ID", 4, 0, 4), ("Group", 4, 4, 4), ("Score", 9, 0, 255)],
        records.len() as u32,
        11 << 8,
    );
    definition[113 + 15] = 2;
    raw::put16(&mut definition, 113 + 7, 0);
    raw::put16(&mut definition, 43, 1);
    // Two12B counter blocks precede all columns; column names precede indexes.
    definition.copy_within(63..168, 87);
    definition[63..87].fill(0);
    raw::put32(&mut definition, 67, histories[0]);
    raw::put32(&mut definition, 79, histories[1]);
    raw::put32(&mut definition, 47, 4);
    raw::put32(&mut definition, 51, 2);
    raw::put32(&mut definition, 59, (11 << 8) | 1);
    let physical = 192;
    definition[physical..physical + 216].fill(0);
    for index in 0..2 {
        let at = physical + index * 52;
        for component in 0..10 {
            raw::put16(&mut definition, at + 4 + component * 3, 0xffff);
        }
        raw::put16(&mut definition, at + 4, index as u16);
        definition[at + 6] = 1;
        raw::put32(&mut definition, at + 34, (15 << 8) | index as u32);
        raw::put32(&mut definition, at + 38, 13 + index as u32);
        definition[at + 46] = if index == 0 {
            0x89
        } else if unique {
            0x81
        } else {
            0x80
        };
    }
    let mut names = 408;
    for (logical, name) in ["PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias"]
        .iter()
        .enumerate()
    {
        let at = 296 + logical * 28;
        raw::put32(&mut definition, at + 4, logical as u32);
        raw::put32(&mut definition, at + 8, (logical / 2) as u32);
        definition[at + 23] = u8::from(logical == 0);
        let wire = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(&mut definition, names, wire.len() as u16);
        names += 2;
        definition[names..names + wire.len()].copy_from_slice(&wire);
        names += wire.len();
    }
    raw::put16(&mut definition, names, 0xffff);
    install(&mut bytes, 10, &definition);
    let data = if large { vec![12, 17] } else { vec![12] };
    let mut rows = Vec::new();
    for (page, chunk) in data.iter().zip(records.chunks(if large { 200 } else { 3 })) {
        let raw_rows = chunk.iter().map(|row| (0, record(row))).collect::<Vec<_>>();
        let mut body = raw::data_page(10, &raw_rows);
        raw::put16(
            &mut body,
            2,
            (PAGE
                - 14
                - raw_rows.len() * 2
                - raw_rows.iter().map(|(_, row)| row.len()).sum::<usize>()) as u16,
        );
        install(&mut bytes, *page, &body);
        rows.extend(
            chunk
                .iter()
                .cloned()
                .enumerate()
                .map(|(slot, row)| (row, (*page << 8) | slot as u32)),
        );
    }
    map_page(&mut bytes, 11, vec![map(data.clone()), map(data.clone())]);
    map_page(&mut bytes, 15, vec![map([13]), map([14])]);
    let eof = (bytes.len() / PAGE) as u32;
    map_page(&mut bytes, 1, vec![map(eof..512)]);
    for index in 0..2 {
        let entries = wires(&rows, index == 1, false);
        let page = leaf(None, &entries, usize::from(large && index == 0), 0, 0);
        install(&mut bytes, 13 + index, &page);
    }
    Case {
        before: bytes,
        rows,
        data,
        histories,
        descending: false,
    }
}

pub fn expected(case: &Case, requests: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    let mut owned = case.data.clone();
    let mut hints = case.data.clone();
    for row in requests {
        let encoded = record(row);
        let candidate = *hints.iter().max().unwrap();
        let start = candidate as usize * PAGE;
        let free = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap()) as usize;
        let page = if free < encoded.len() + 2 {
            hints.retain(|number| *number != candidate);
            let next = (bytes.len() / PAGE) as u32;
            bytes.resize(bytes.len() + PAGE, 0);
            owned.push(next);
            hints.push(next);
            install(&mut bytes, next, &raw::data_page(10, &[]));
            next
        } else {
            candidate
        };
        let start = page as usize * PAGE;
        let count = u16::from_le_bytes(bytes[start + 12..start + 14].try_into().unwrap()) as usize;
        let end = if count == 0 {
            PAGE
        } else {
            u16::from_le_bytes(
                bytes[start + 12 + count * 2..start + 14 + count * 2]
                    .try_into()
                    .unwrap(),
            ) as usize
        };
        let at = end - encoded.len();
        bytes[start + at..start + end].copy_from_slice(&encoded);
        raw::put16(&mut bytes, start + 14 + count * 2, at as u16);
        raw::put16(&mut bytes, start + 12, (count + 1) as u16);
        raw::put16(&mut bytes, start + 2, (at - 14 - 2 * (count + 1)) as u16);
        rows.push((row.clone(), (page << 8) | count as u32));
    }
    let mut members = [vec![13], vec![14]];
    for (index, members) in members.iter_mut().enumerate() {
        let entries = wires(&rows, index == 1, case.descending);
        let old = &case.before[(13 + index) * PAGE..(14 + index) * PAGE];
        let seed = u16::from_le_bytes(old[24..26].try_into().unwrap()) as usize;
        if framing(&entries, seed).1 <= 3616 {
            install(
                &mut bytes,
                13 + index as u32,
                &leaf(Some(old), &entries, seed, 0, 0),
            );
        } else {
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = entries.len() / 2;
            install(
                &mut bytes,
                right,
                &leaf(None, &entries[middle..], 0, left, 0),
            );
            install(
                &mut bytes,
                left,
                &leaf(None, &entries[..middle], 0, 0, right),
            );
            install(
                &mut bytes,
                13 + index as u32,
                &root(old, left, right, &entries[middle - 1]),
            );
            members.extend([right, left]);
        }
    }
    let mut known = case
        .rows
        .iter()
        .map(|(row, _)| row.group)
        .collect::<BTreeSet<_>>();
    let mut secondary_delta = 0;
    for row in requests {
        secondary_delta += u32::from(known.insert(row.group));
    }
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    raw::put32(
        &mut bytes,
        10 * PAGE + 67,
        case.histories[0] + requests.len() as u32,
    );
    raw::put32(
        &mut bytes,
        10 * PAGE + 79,
        case.histories[1] + secondary_delta,
    );
    map_page(&mut bytes, 11, vec![map(owned), map(hints)]);
    map_page(&mut bytes, 15, members.into_iter().map(map).collect());
    let eof = (bytes.len() / PAGE) as u32;
    map_page(&mut bytes, 1, vec![map(eof..512)]);
    bytes
}

pub fn descending(mut case: Case) -> Case {
    case.descending = true;
    case.before[10 * PAGE + 250] = 0;
    let entries = wires(&case.rows, true, true);
    install(&mut case.before, 14, &leaf(None, &entries, 0, 0, 0));
    case
}

pub fn ignored_null_source(code: u8) -> Case {
    let mut case = source(code, false, false);
    case.before[10 * PAGE + 290] = 0x82;
    let entries = wires(&case.rows, true, false)
        .into_iter()
        .filter(|wire| wire[0] != 0)
        .collect::<Vec<_>>();
    install(&mut case.before, 14, &leaf(None, &entries, 0, 0, 0));
    case
}

/// Each physical index is independently readable, but their page ownership is
/// deliberately shared. Distinct usage records do not prove tree exclusivity.
pub fn shared_root_source(code: u8) -> Case {
    let mut case = source(code, false, false);
    for (row, _) in &mut case.rows {
        row.group = Some(row.id);
    }
    let records = case
        .rows
        .iter()
        .map(|(row, _)| (0, record(row)))
        .collect::<Vec<_>>();
    let free =
        PAGE - 14 - 2 * records.len() - records.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
    let mut data = raw::data_page(10, &records);
    raw::put16(&mut data, 2, free as u16);
    install(&mut case.before, 12, &data);
    // The primary13 key/ROW stream now describes both actual columns. The old
    // secondary14 remains allocated and untouched as an orphan source page.
    raw::put32(&mut case.before, 10 * PAGE + 282, 13);
    map_page(&mut case.before, 15, vec![map([13]), map([13])]);
    case
}
