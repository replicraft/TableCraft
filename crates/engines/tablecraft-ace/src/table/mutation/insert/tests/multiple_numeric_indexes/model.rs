//! Width-aware literal DATA and key wires; shared fixture framing has no codec.
use super::super::{model as original, multiple_indexes::model as frames};
use std::collections::BTreeSet;
use tablecraft_core::values::{Currency, Value};
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;
const GROUP_HEADER: usize = 10 * PAGE + 112;

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Byte,
    Int16,
    Int64,
    Currency,
}

impl Kind {
    pub(super) fn bounds(self) -> (i64, i64) {
        match self {
            Self::Byte => (0, 255),
            Self::Int16 => (i64::from(i16::MIN), i64::from(i16::MAX)),
            Self::Int64 | Self::Currency => (i64::MIN, i64::MAX),
        }
    }

    fn declaration(self) -> (u8, usize) {
        match self {
            Self::Byte => (2, 1),
            Self::Int16 => (3, 2),
            Self::Int64 => (0x13, 8),
            Self::Currency => (5, 8),
        }
    }

    fn value(self, number: i64) -> Value {
        match self {
            Self::Byte => Value::Byte(u8::try_from(number).unwrap()),
            Self::Int16 => Value::Int16(i16::try_from(number).unwrap()),
            Self::Int64 => Value::Int64(number),
            Self::Currency => Value::Currency(Currency::from_scaled(number)),
        }
    }
}

#[derive(Clone)]
pub(super) struct Row {
    pub id: i32,
    pub group: Option<i64>,
}

impl Row {
    pub(super) fn values(&self, kind: Kind) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            self.group.map_or(Value::Null, |number| kind.value(number)),
            Value::Binary(Vec::new()),
        ]
    }
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<(Row, u32)>,
    pub data: Vec<u32>,
    pub kind: Kind,
    pub descending: bool,
    pub histories: [u32; 2],
}

fn record(row: &Row, kind: Kind) -> Vec<u8> {
    let width = kind.declaration().1;
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(&row.group.unwrap_or(0).to_le_bytes()[..width]);
    let end = bytes.len() as u16;
    // Empty Binary is present, and its start and end coincide after fixed data.
    bytes.extend(end.to_le_bytes());
    bytes.extend(end.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.push(5 | (u8::from(row.group.is_some()) << 1));
    bytes
}

fn wires(rows: &[(Row, u32)], kind: Kind, secondary: bool, descending: bool) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .map(|(row, reference)| {
            let number = if secondary {
                row.group
            } else {
                Some(i64::from(row.id))
            };
            let mut wire = match number {
                None => vec![0],
                Some(number) => {
                    let width = if secondary { kind.declaration().1 } else { 4 };
                    let mut bytes = number.to_be_bytes()[8 - width..].to_vec();
                    if !secondary || !matches!(kind, Kind::Byte) {
                        bytes[0] ^= 0x80;
                    }
                    let mut wire = vec![127];
                    wire.extend(bytes);
                    wire
                }
            };
            if secondary && descending {
                if number.is_none() {
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

pub(super) fn source(code: u8, kind: Kind, descending: bool, large: bool) -> Case {
    let baseline = frames::source(code, false, false);
    let mut before = baseline.before;
    before.resize(if large { 18 } else { 16 } * PAGE, 0);
    let (native, width) = kind.declaration();
    before[GROUP_HEADER] = native;
    raw::put16(&mut before, GROUP_HEADER + 23, width as u16);
    before[10 * PAGE + 250] = u8::from(!descending);
    let (minimum, maximum) = kind.bounds();
    let records = if large {
        assert!(matches!(kind, Kind::Int64 | Kind::Currency));
        (0..300)
            .map(|position| Row {
                id: position + 1,
                group: Some(
                    (i128::from(minimum)
                        + (i128::from(maximum) - i128::from(minimum)) * i128::from(position) / 299)
                        as i64,
                ),
            })
            .collect::<Vec<_>>()
    } else {
        vec![
            Row {
                id: 1,
                group: Some(minimum),
            },
            Row {
                id: 2,
                group: Some(maximum),
            },
            Row { id: 3, group: None },
        ]
    };
    let data = if large { vec![12, 17] } else { vec![12] };
    let mut rows = Vec::new();
    for (page, chunk) in data.iter().zip(records.chunks(if large { 150 } else { 3 })) {
        let encoded = chunk
            .iter()
            .map(|row| (0, record(row, kind)))
            .collect::<Vec<_>>();
        let mut body = raw::data_page(10, &encoded);
        raw::put16(
            &mut body,
            2,
            (PAGE
                - 14
                - 2 * chunk.len()
                - encoded.iter().map(|(_, bytes)| bytes.len()).sum::<usize>()) as u16,
        );
        frames::install(&mut before, *page, &body);
        rows.extend(
            chunk
                .iter()
                .cloned()
                .enumerate()
                .map(|(slot, row)| (row, (*page << 8) | slot as u32)),
        );
    }
    raw::put32(&mut before, 10 * PAGE + 16, rows.len() as u32);
    frames::map_page(
        &mut before,
        11,
        vec![frames::map(data.clone()), frames::map(data.clone())],
    );
    let eof = (before.len() / PAGE) as u32;
    frames::map_page(&mut before, 1, vec![frames::map(eof..512)]);
    for physical in 0..2 {
        let entries = wires(&rows, kind, physical == 1, descending);
        frames::install(
            &mut before,
            13 + physical,
            &frames::leaf(None, &entries, usize::from(large && physical == 1), 0, 0),
        );
    }
    let histories = if large { [317, 629] } else { [17, 29] };
    raw::put32(&mut before, 10 * PAGE + 67, histories[0]);
    raw::put32(&mut before, 10 * PAGE + 79, histories[1]);
    Case {
        before,
        rows,
        data,
        kind,
        descending,
        histories,
    }
}

pub(super) fn expected(case: &Case, requests: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    let mut owned = case.data.clone();
    let mut hints = case.data.clone();
    for row in requests {
        let encoded = record(row, case.kind);
        let candidate = *hints.iter().max().unwrap();
        let free = u16::from_le_bytes(
            bytes[candidate as usize * PAGE + 2..candidate as usize * PAGE + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let page = if free < encoded.len() + 2 {
            hints.retain(|page| *page != candidate);
            let next = (bytes.len() / PAGE) as u32;
            bytes.resize(bytes.len() + PAGE, 0);
            owned.push(next);
            hints.push(next);
            frames::install(&mut bytes, next, &raw::data_page(10, &[]));
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
    for (physical, members) in members.iter_mut().enumerate() {
        let entries = wires(&rows, case.kind, physical == 1, case.descending);
        let old = &case.before[(13 + physical) * PAGE..(14 + physical) * PAGE];
        let seed = u16::from_le_bytes(old[24..26].try_into().unwrap()) as usize;
        if frames::framing(&entries, seed).1 <= 3616 {
            frames::install(
                &mut bytes,
                13 + physical as u32,
                &frames::leaf(Some(old), &entries, seed, 0, 0),
            );
        } else {
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = entries.len() / 2;
            frames::install(
                &mut bytes,
                right,
                &frames::leaf(None, &entries[middle..], 0, left, 0),
            );
            frames::install(
                &mut bytes,
                left,
                &frames::leaf(None, &entries[..middle], 0, 0, right),
            );
            frames::install(
                &mut bytes,
                13 + physical as u32,
                &frames::root(old, left, right, &entries[middle - 1]),
            );
            members.extend([right, left]);
        }
    }
    let mut known = case
        .rows
        .iter()
        .map(|(row, _)| row.group)
        .collect::<BTreeSet<_>>();
    let delta = requests
        .iter()
        .filter(|row| known.insert(row.group))
        .count();
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    raw::put32(
        &mut bytes,
        10 * PAGE + 67,
        case.histories[0] + requests.len() as u32,
    );
    raw::put32(&mut bytes, 10 * PAGE + 79, case.histories[1] + delta as u32);
    frames::map_page(&mut bytes, 11, vec![frames::map(owned), frames::map(hints)]);
    frames::map_page(
        &mut bytes,
        15,
        members.into_iter().map(frames::map).collect(),
    );
    let eof = (bytes.len() / PAGE) as u32;
    frames::map_page(&mut bytes, 1, vec![frames::map(eof..512)]);
    bytes
}

pub(super) fn assert_typed(bytes: &[u8], expected: &[(Row, u32)], kind: Kind) {
    let snapshot = crate::read_table(
        &mut std::io::Cursor::new(bytes),
        tablecraft_model::CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    for ((row, _), actual) in expected.iter().zip(snapshot.rows()) {
        assert_eq!(actual, &row.values(kind));
    }
}

/// Move physical ownership while retaining each logical declaration and ROW.
/// This is only used for fitted images, whose physical processing order creates
/// no suffix allocation ordering distinction.
pub(super) fn transpose_physical_owners(bytes: &mut [u8]) {
    let definition = 10 * PAGE;
    for (first, second, width) in [(63, 75, 12), (192, 244, 52)] {
        let left = bytes[definition + first..definition + first + width].to_vec();
        let right = bytes[definition + second..definition + second + width].to_vec();
        bytes[definition + first..definition + first + width].copy_from_slice(&right);
        bytes[definition + second..definition + second + width].copy_from_slice(&left);
    }
    for logical in 0..4 {
        let offset = definition + 296 + logical * 28 + 8;
        let physical = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        assert!(physical < 2);
        raw::put32(bytes, offset, 1 - physical);
    }
}
