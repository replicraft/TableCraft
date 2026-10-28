//! Literal mask/IEEE/DATE records and index wires, separate from native codecs.
use super::super::{model as original, multiple_indexes::model as frames};
use std::collections::BTreeSet;
use tablecraft_core::values::{Double, LegacyDateTime, Single, Value};
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;
const GROUP_HEADER: usize = 10 * PAGE + 112;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Boolean,
    Single,
    Double,
    Date,
}

impl Kind {
    fn declaration(self) -> (u8, usize) {
        match self {
            Self::Boolean => (1, 1),
            Self::Single => (6, 4),
            Self::Double => (7, 8),
            Self::Date => (8, 8),
        }
    }

    fn physical_width(self) -> usize {
        if self == Self::Boolean {
            0
        } else {
            self.declaration().1
        }
    }

    pub(super) fn negative_zero(self) -> u64 {
        if self == Self::Single {
            0x80000000
        } else {
            0x8000000000000000
        }
    }

    fn value(self, bits: u64) -> Value {
        match self {
            Self::Boolean => {
                assert!(bits < 2);
                Value::Boolean(bits == 1)
            }
            Self::Single => Value::Single(Single::from_bits(u32::try_from(bits).unwrap())),
            Self::Double => Value::Double(Double::from_bits(bits)),
            Self::Date => Value::DateTime(LegacyDateTime::from_bits(bits).unwrap()),
        }
    }

    fn key(self, bits: u64) -> Vec<u8> {
        if self == Self::Boolean {
            assert!(bits < 2);
            return vec![127, if bits == 1 { 0 } else { 255 }];
        }
        let width = self.physical_width();
        let sign = 1_u64 << (width * 8 - 1);
        let ordered = if bits & sign != 0 { !bits } else { bits ^ sign };
        let mut wire = vec![127];
        wire.extend(&ordered.to_be_bytes()[8 - width..]);
        wire
    }
}

#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub kind: Kind,
    pub descending: bool,
    pub unique: bool,
    pub ignore_nulls: bool,
    pub required: bool,
}

impl Settings {
    pub(super) fn ordinary(kind: Kind, descending: bool) -> Self {
        Self {
            kind,
            descending,
            unique: false,
            ignore_nulls: false,
            required: false,
        }
    }
}

#[derive(Clone)]
pub(super) struct Row {
    pub id: i32,
    pub group: Option<u64>,
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
    pub settings: Settings,
    pub histories: [u32; 2],
}

fn record(row: &Row, kind: Kind) -> Vec<u8> {
    let width = kind.physical_width();
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(&row.group.unwrap_or(0).to_le_bytes()[..width]);
    let end = bytes.len() as u16;
    // Empty Binary is present, and its start and end coincide after fixed data.
    bytes.extend(end.to_le_bytes());
    bytes.extend(end.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    let group_bit = if kind == Kind::Boolean {
        assert!(row.group.is_some_and(|bits| bits < 2));
        row.group == Some(1)
    } else {
        row.group.is_some()
    };
    bytes.push(5 | (u8::from(group_bit) << 1));
    bytes
}

fn wires(rows: &[(Row, u32)], kind: Kind, secondary: bool, descending: bool) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .map(|(row, reference)| {
            let number = if secondary {
                row.group
            } else {
                Some(row.id as u32 as u64)
            };
            let mut wire = if secondary {
                number.map_or_else(|| vec![0], |bits| kind.key(bits))
            } else {
                let mut wire = vec![127];
                wire.extend(((row.id as u32) ^ 0x80000000).to_be_bytes());
                wire
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

pub(super) fn source(code: u8, settings: Settings, pressure: bool) -> Case {
    let baseline = frames::source(code, false, false);
    let mut before = baseline.before;
    before.resize(if pressure { 18 } else { 16 } * PAGE, 0);
    let (native, width) = settings.kind.declaration();
    before[GROUP_HEADER] = native;
    raw::put16(&mut before, GROUP_HEADER + 23, width as u16);
    before[10 * PAGE + 250] = u8::from(!settings.descending);
    before[10 * PAGE + 290] = 0x80
        | u8::from(settings.unique)
        | (u8::from(settings.ignore_nulls) << 1)
        | (u8::from(settings.required) << 3);
    let values = if pressure {
        assert!(!settings.required && !settings.ignore_nulls && settings.kind != Kind::Boolean);
        let count = if settings.kind == Kind::Single {
            400
        } else {
            277
        };
        let first = if settings.kind == Kind::Single {
            0x3f000000
        } else {
            0x3fe0000000000000
        };
        std::iter::once(None)
            .chain((0..count).map(|position| Some(first + position)))
            .collect::<Vec<_>>()
    } else if settings.kind == Kind::Boolean {
        if settings.unique {
            vec![Some(1), Some(0)]
        } else {
            vec![Some(1), Some(0), Some(1)]
        }
    } else {
        vec![
            Some(settings.kind.negative_zero()),
            Some(0),
            if settings.required {
                Some(if settings.kind == Kind::Single {
                    0x3f000000
                } else {
                    0x3fe0000000000000
                })
            } else {
                None
            },
        ]
    };
    let records = values
        .into_iter()
        .enumerate()
        .map(|(position, group)| Row {
            id: position as i32 + 1,
            group,
        })
        .collect::<Vec<_>>();
    let data = if pressure { vec![12, 17] } else { vec![12] };
    let mut rows = Vec::new();
    for (page, chunk) in data.iter().zip(records.chunks(if pressure {
        records.len().div_ceil(2)
    } else {
        records.len()
    })) {
        let encoded = chunk
            .iter()
            .map(|row| (0, record(row, settings.kind)))
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
        let mut entries = wires(&rows, settings.kind, physical == 1, settings.descending);
        if physical == 1 && settings.ignore_nulls {
            let marker = if settings.descending { 255 } else { 0 };
            entries.retain(|wire| wire[0] != marker);
        }
        frames::install(
            &mut before,
            13 + physical,
            &frames::leaf(None, &entries, 0, 0, 0),
        );
    }
    let histories = if pressure { [417, 629] } else { [17, 29] };
    raw::put32(&mut before, 10 * PAGE + 67, histories[0]);
    raw::put32(&mut before, 10 * PAGE + 79, histories[1]);
    Case {
        before,
        rows,
        data,
        settings,
        histories,
    }
}

pub(super) fn expected(case: &Case, requests: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    let mut owned = case.data.clone();
    let mut hints = case.data.clone();
    for row in requests {
        let encoded = record(row, case.settings.kind);
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
        let mut entries = wires(
            &rows,
            case.settings.kind,
            physical == 1,
            case.settings.descending,
        );
        if physical == 1 && case.settings.ignore_nulls {
            let null_marker = if case.settings.descending { 255 } else { 0 };
            entries.retain(|entry| entry[0] != null_marker);
        }
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
        .filter(|row| {
            (!case.settings.ignore_nulls || row.group.is_some()) && known.insert(row.group)
        })
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
