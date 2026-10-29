//! Literal fixed timestamps and whole images; no producer timestamp encoder.
use super::super::multiple_indexes::model as frames;
use std::collections::BTreeSet;
use tablecraft_core::values::{ExtendedDateTime, Value};
use tablecraft_testkit::access as raw;

pub const PAGE: usize = 4096;

#[derive(Clone, Copy)]
pub enum Timestamp {
    Epoch,
    Leap,
    Maximum,
    Tick,
}

impl Timestamp {
    pub fn value(self) -> ExtendedDateTime {
        let (date, time) = match self {
            Self::Epoch => ((1, 1, 1), (0, 0, 0, 0)),
            Self::Leap => ((2000, 2, 29), (12, 34, 56, 1_234_567)),
            Self::Maximum => ((9999, 12, 31), (23, 59, 59, 9_999_999)),
            Self::Tick => ((1, 1, 1), (0, 0, 0, 1)),
        };
        ExtendedDateTime::new(date.0, date.1, date.2, time.0, time.1, time.2, time.3).unwrap()
    }

    fn bytes(self) -> Vec<u8> {
        let (days, seconds, fraction) = match self {
            Self::Epoch => (0, 0, 0),
            Self::Leap => (730_178, 45_296, 1_234_567),
            Self::Maximum => (3_652_058, 86_399, 9_999_999),
            Self::Tick => (0, 0, 1),
        };
        let bytes = format!("{days:019}:{seconds:012}{fraction:07}:7\0").into_bytes();
        assert_eq!(bytes.len(), 42);
        bytes
    }
}

#[derive(Clone)]
pub struct Row {
    pub id: i32,
    pub group: Option<i32>,
    pub timestamp: Option<Timestamp>,
}

impl Row {
    pub fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            self.group.map_or(Value::Null, Value::Int32),
            self.timestamp
                .map_or(Value::Null, |value| Value::DateTimeExtended(value.value())),
        ]
    }
}

pub struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<Row>,
}

fn record(row: &Row, opaque_null: bool) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.group.unwrap_or(0).to_le_bytes());
    if let Some(timestamp) = row.timestamp {
        bytes.extend(timestamp.bytes());
    } else {
        bytes.extend([if opaque_null { 0xa5 } else { 0 }; 42]);
    }
    bytes.push(1 | (u8::from(row.group.is_some()) << 1) | (u8::from(row.timestamp.is_some()) << 2));
    assert_eq!(bytes.len(), 53);
    bytes
}

fn entries(rows: &[(Row, u32)], secondary: bool) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .map(|(row, reference)| {
            let key = if secondary { row.group } else { Some(row.id) };
            let mut bytes = if let Some(value) = key {
                let mut bytes = vec![127];
                bytes.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                bytes
            } else {
                vec![0]
            };
            bytes.extend(reference.to_be_bytes());
            bytes
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

pub fn source(code: u8) -> Case {
    let mut before = frames::source(code, false, false).before;
    let column = 10 * PAGE + 137;
    before[column] = 20;
    before[column + 15] = 3;
    raw::put16(&mut before, column + 21, 8);
    raw::put16(&mut before, column + 23, 42);
    raw::put16(&mut before, 10 * PAGE + 43, 0);
    let rows = vec![
        Row {
            id: 1,
            group: Some(10),
            timestamp: Some(Timestamp::Epoch),
        },
        Row {
            id: 2,
            group: Some(10),
            timestamp: None,
        },
        Row {
            id: 3,
            group: None,
            timestamp: Some(Timestamp::Maximum),
        },
    ];
    let records = rows
        .iter()
        .map(|row| (0, record(row, true)))
        .collect::<Vec<_>>();
    let mut page = raw::data_page(10, &records);
    raw::put16(&mut page, 2, (PAGE - 14 - rows.len() * 55) as u16);
    frames::install(&mut before, 12, &page);
    Case { before, rows }
}

pub fn expected(case: &Case, added: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case
        .rows
        .iter()
        .cloned()
        .enumerate()
        .map(|(slot, row)| (row, (12 << 8) | slot as u32))
        .collect::<Vec<_>>();
    for row in added {
        let page = 12 * PAGE;
        let count = u16::from_le_bytes(bytes[page + 12..page + 14].try_into().unwrap()) as usize;
        let end = u16::from_le_bytes(
            bytes[page + 12 + count * 2..page + 14 + count * 2]
                .try_into()
                .unwrap(),
        ) as usize;
        let wire = record(row, false);
        let start = end - wire.len();
        assert!(start >= 14 + (count + 1) * 2);
        bytes[page + start..page + end].copy_from_slice(&wire);
        raw::put16(&mut bytes, page + 14 + count * 2, start as u16);
        raw::put16(&mut bytes, page + 12, (count + 1) as u16);
        raw::put16(&mut bytes, page + 2, (start - 14 - (count + 1) * 2) as u16);
        rows.push((row.clone(), (12 << 8) | count as u32));
    }
    for physical in 0..2 {
        let root = 13 + physical;
        let original = &case.before[root * PAGE..(root + 1) * PAGE];
        let page = frames::leaf(Some(original), &entries(&rows, physical == 1), 0, 0, 0);
        frames::install(&mut bytes, root as u32, &page);
    }
    let mut known = case
        .rows
        .iter()
        .map(|row| row.group)
        .collect::<BTreeSet<_>>();
    let delta = added.iter().filter(|row| known.insert(row.group)).count();
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    raw::put32(&mut bytes, 10 * PAGE + 67, 17 + added.len() as u32);
    raw::put32(&mut bytes, 10 * PAGE + 79, 29 + delta as u32);
    bytes
}
