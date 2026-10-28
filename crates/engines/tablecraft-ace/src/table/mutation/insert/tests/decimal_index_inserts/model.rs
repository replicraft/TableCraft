//! Independent Decimal fixtures supply FIELD and KEY bytes; framing stays shared.
use super::super::multiple_indexes::model as frames;
use crate::test_support::base;
use std::collections::BTreeSet;
use tablecraft_core::values::{Decimal, Guid, Value};
use tablecraft_testkit::access as support;

#[allow(dead_code)]
pub(super) mod numeric_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/composite_indexes/fixture.rs"
    ));
}

#[allow(dead_code)]
mod decimal_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/decimal_indexes/fixture.rs"
    ));
}

pub(super) const PAGE: usize = 4096;

#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub tuple: bool,
    pub descending: bool,
    pub reversed: bool,
    pub unique: bool,
    pub ignored: bool,
    pub required: bool,
    pub precision: u8,
    pub scale: u8,
}

impl Settings {
    pub fn ordinary(tuple: bool, descending: bool) -> Self {
        Self {
            tuple,
            descending,
            reversed: false,
            unique: false,
            ignored: false,
            required: false,
            precision: 28,
            scale: 4,
        }
    }
}

#[derive(Clone)]
pub(super) struct Row {
    pub id: i32,
    pub amount: Option<i128>,
    pub guid: Option<[u8; 16]>,
}

impl Row {
    pub fn values(&self, settings: Settings) -> Vec<Value> {
        let mut values = vec![
            Value::Int32(self.id),
            self.amount.map_or(Value::Null, |number| {
                Value::Decimal(Decimal::new(number, settings.scale).unwrap())
            }),
        ];
        if settings.tuple {
            values.push(
                self.guid
                    .map_or(Value::Null, |bytes| Value::Guid(Guid::from_bytes(bytes))),
            );
        }
        values
    }
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<(Row, u32)>,
    pub data: Vec<u32>,
    pub settings: Settings,
}

pub(super) fn guid(seed: u8) -> [u8; 16] {
    [seed, 1, 2, 3, 4, 5, 6, 7, 128, 9, 10, 11, 12, 13, 14, 255]
}

// Reuse the existing independent Decimal record/index owner, never its codec.
fn decimal(value: Option<i128>, settings: Settings) -> (Vec<u8>, Vec<u8>) {
    let bytes = decimal_fixture::database(
        settings.descending,
        settings.precision,
        settings.scale,
        &[value],
        0,
    );
    let data = &bytes[12 * PAGE..13 * PAGE];
    let start = u16::from_le_bytes(data[14..16].try_into().unwrap()) as usize;
    let record = &data[start..];
    let payload = if value.is_some() {
        record[6..23].to_vec()
    } else {
        Vec::new()
    };
    let width = if value.is_some() { 18 } else { 1 };
    (
        payload,
        bytes[14 * PAGE + 480..14 * PAGE + 480 + width].to_vec(),
    )
}

fn record(row: &Row, settings: Settings, null_fill: u8) -> Vec<u8> {
    let mut bytes = (if settings.tuple { 3_u16 } else { 2_u16 })
        .to_le_bytes()
        .to_vec();
    bytes.extend(row.id.to_le_bytes());
    if settings.tuple {
        let mut stored = row.guid.unwrap_or([null_fill; 16]);
        stored[..4].reverse();
        stored[4..6].reverse();
        stored[6..8].reverse();
        bytes.extend(stored);
    }
    let start = bytes.len() as u16;
    bytes.extend(decimal(row.amount, settings).0);
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend(start.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.push(
        1 | (u8::from(row.amount.is_some()) << 1)
            | (u8::from(settings.tuple && row.guid.is_some()) << 2),
    );
    bytes
}

fn guid_key(value: Option<[u8; 16]>, descending: bool) -> Vec<u8> {
    let Some(value) = value else {
        return vec![if descending { 255 } else { 0 }];
    };
    let mut bytes = vec![if descending { 128 } else { 127 }];
    bytes.extend(
        value[..8]
            .iter()
            .map(|byte| if descending { !byte } else { *byte }),
    );
    bytes.push(9);
    bytes.extend(
        value[8..]
            .iter()
            .map(|byte| if descending { !byte } else { *byte }),
    );
    bytes.push(if descending { 247 } else { 8 });
    bytes
}

fn entries(rows: &[(Row, u32)], settings: Settings, secondary: bool) -> Vec<Vec<u8>> {
    let mut entries = Vec::new();
    for (row, reference) in rows {
        let mut key = if secondary {
            if settings.ignored && row.amount.is_none() && (!settings.tuple || row.guid.is_none()) {
                continue;
            }
            let amount = decimal(row.amount, settings).1;
            if settings.tuple {
                let guid = guid_key(row.guid, !settings.descending);
                if settings.reversed {
                    [guid, amount].concat()
                } else {
                    [amount, guid].concat()
                }
            } else {
                amount
            }
        } else {
            [
                vec![127],
                ((row.id as u32) ^ 0x8000_0000).to_be_bytes().to_vec(),
            ]
            .concat()
        };
        key.extend(reference.to_be_bytes());
        entries.push(key);
    }
    entries.sort();
    entries
}

fn definition(rows: usize, settings: Settings) -> Vec<u8> {
    // Score retains the baseline's inert Caption property binding.
    let fields = if settings.tuple {
        vec![("ID", 4, 0, 4), ("Amount", 16, 0, 17), ("Score", 15, 4, 16)]
    } else {
        vec![("ID", 4, 0, 4), ("Score", 16, 0, 17)]
    };
    let mut bytes = base::definition(&fields, rows as u32, 11 << 8);
    support::put16(&mut bytes, 43, 1);
    let field = 63 + 25;
    bytes[field + 11] = settings.precision;
    bytes[field + 12] = settings.scale;
    bytes[field + 15] = 2;
    support::put16(&mut bytes, field + 7, 0);
    let end = 63
        + fields.len() * 25
        + fields
            .iter()
            .map(|(name, _, _, _)| 2 + name.encode_utf16().count() * 2)
            .sum::<usize>();
    bytes.copy_within(63..end, 87);
    bytes[63..87].fill(0);
    support::put32(&mut bytes, 67, 337);
    support::put32(&mut bytes, 79, 659);
    support::put32(&mut bytes, 47, 4);
    support::put32(&mut bytes, 51, 2);
    support::put32(&mut bytes, 59, (11 << 8) | 1);
    let physical = end + 24;
    let logical = physical + 104;
    let mut names = logical + 112;
    bytes[physical..names].fill(0);
    for index in 0..2 {
        let at = physical + index * 52;
        for position in 0..10 {
            support::put16(&mut bytes, at + 4 + position * 3, 0xffff);
        }
        let columns = if index == 0 {
            vec![(0, false)]
        } else if settings.tuple && settings.reversed {
            vec![(2, !settings.descending), (1, settings.descending)]
        } else if settings.tuple {
            vec![(1, settings.descending), (2, !settings.descending)]
        } else {
            vec![(1, settings.descending)]
        };
        for (position, (column, descending)) in columns.into_iter().enumerate() {
            support::put16(&mut bytes, at + 4 + position * 3, column);
            bytes[at + 6 + position * 3] = u8::from(!descending);
        }
        support::put32(&mut bytes, at + 34, (15 << 8) | index as u32);
        support::put32(&mut bytes, at + 38, 13 + index as u32);
        bytes[at + 46] = if index == 0 {
            0x89
        } else {
            0x80 | u8::from(settings.unique)
                | (u8::from(settings.ignored) << 1)
                | (u8::from(settings.required) << 3)
        };
    }
    for (position, name) in ["PrimaryID", "PrimaryAlias", "DecimalOrder", "DecimalAlias"]
        .iter()
        .enumerate()
    {
        let at = logical + position * 28;
        support::put32(&mut bytes, at + 4, position as u32);
        support::put32(&mut bytes, at + 8, (position / 2) as u32);
        bytes[at + 23] = u8::from(position == 0);
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut bytes, names, encoded.len() as u16);
        names += 2;
        bytes[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    support::put16(&mut bytes, names, 0xffff);
    bytes
}

pub(super) fn source(code: u8, settings: Settings, pressure: bool) -> Case {
    let count = if pressure {
        match (settings.tuple, settings.ignored) {
            (false, false) => 164,
            (false, true) => 172,
            (true, false) => 88,
            (true, true) => 90,
        }
    } else {
        2
    };
    let limit = 10_i128.pow(u32::from(settings.precision)) - 1;
    let mut records = (0..count)
        .map(|position| Row {
            id: position + 1,
            amount: Some(if pressure {
                -limit + (2 * limit / i128::from(count - 1)) * i128::from(position)
            } else if position == 0 {
                -limit
            } else {
                limit
            }),
            guid: settings.tuple.then(|| guid(position as u8)),
        })
        .collect::<Vec<_>>();
    if !settings.required {
        records.push(Row {
            id: count + 1,
            amount: None,
            guid: None,
        });
    }
    let data = if pressure { vec![12, 17] } else { vec![12] };
    let mut before = frames::source(code, false, false).before;
    before.resize(if pressure { 18 } else { 16 } * PAGE, 0);
    frames::install(&mut before, 10, &definition(records.len(), settings));
    let per_page = records.len().div_ceil(data.len());
    let mut rows = Vec::new();
    for (page, chunk) in data.iter().zip(records.chunks(per_page)) {
        let encoded = chunk
            .iter()
            .map(|row| (0, record(row, settings, 0x5e)))
            .collect::<Vec<_>>();
        let free = PAGE
            - 14
            - 2 * chunk.len()
            - encoded.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
        let mut body = support::data_page(10, &encoded);
        support::put16(&mut body, 2, free as u16);
        frames::install(&mut before, *page, &body);
        rows.extend(
            chunk
                .iter()
                .cloned()
                .enumerate()
                .map(|(slot, row)| (row, (*page << 8) | slot as u32)),
        );
    }
    frames::map_page(
        &mut before,
        11,
        vec![frames::map(data.clone()), frames::map(data.clone())],
    );
    let eof = (before.len() / PAGE) as u32;
    frames::map_page(&mut before, 1, vec![frames::map(eof..512)]);
    let mut case = Case {
        before,
        rows,
        data,
        settings,
    };
    for index in 0..2 {
        let keys = entries(&case.rows, settings, index == 1);
        let seed = usize::from(pressure && settings.ignored && index == 1);
        let active = frames::framing(&keys, seed).1;
        let mut leaf = frames::leaf(None, &keys, seed, 0, 0);
        leaf[480 + active..].fill(if index == 0 { 0xb6 } else { 0xc7 });
        frames::install(&mut case.before, 13 + index, &leaf);
    }
    case
}

pub(super) fn expected(case: &Case, added: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    let page = *case.data.last().unwrap();
    for row in added {
        let base = page as usize * PAGE;
        let count = u16::from_le_bytes(bytes[base + 12..base + 14].try_into().unwrap()) as usize;
        let end = u16::from_le_bytes(
            bytes[base + 12 + count * 2..base + 14 + count * 2]
                .try_into()
                .unwrap(),
        ) as usize;
        let body = record(row, case.settings, 0);
        let at = end - body.len();
        assert!(at >= 14 + 2 * (count + 1));
        bytes[base + at..base + end].copy_from_slice(&body);
        support::put16(&mut bytes, base + 14 + count * 2, at as u16);
        support::put16(&mut bytes, base + 12, (count + 1) as u16);
        support::put16(&mut bytes, base + 2, (at - 14 - 2 * (count + 1)) as u16);
        rows.push((row.clone(), (page << 8) | count as u32));
    }
    let mut owned = vec![14];
    for index in 0..2 {
        let keys = entries(&rows, case.settings, index == 1);
        let old_keys = entries(&case.rows, case.settings, index == 1);
        let old = &case.before[(13 + index) * PAGE..(14 + index) * PAGE];
        if keys == old_keys {
            continue;
        }
        let seed = usize::from(u16::from_le_bytes(old[24..26].try_into().unwrap()));
        if frames::framing(&keys, seed).1 <= 3616 {
            frames::install(
                &mut bytes,
                13 + index as u32,
                &frames::leaf(Some(old), &keys, seed, 0, 0),
            );
        } else {
            assert_eq!(index, 1);
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = keys.len() / 2;
            frames::install(
                &mut bytes,
                right,
                &frames::leaf(None, &keys[middle..], 0, left, 0),
            );
            frames::install(
                &mut bytes,
                left,
                &frames::leaf(None, &keys[..middle], 0, 0, right),
            );
            frames::install(
                &mut bytes,
                14,
                &frames::root(old, left, right, &keys[middle - 1]),
            );
            owned.extend([right, left]);
        }
        let families = |entries: &[Vec<u8>]| {
            entries
                .iter()
                .map(|entry| entry[..entry.len() - 4].to_vec())
                .collect::<BTreeSet<_>>()
        };
        let old = families(&old_keys);
        let delta = families(&keys).difference(&old).count();
        support::put32(
            &mut bytes,
            10 * PAGE + 67 + index * 12,
            (if index == 0 { 337 } else { 659 }) + delta as u32,
        );
    }
    support::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    frames::map_page(&mut bytes, 15, vec![frames::map([13]), frames::map(owned)]);
    let eof = (bytes.len() / PAGE) as u32;
    frames::map_page(&mut bytes, 1, vec![frames::map(eof..512)]);
    bytes
}
