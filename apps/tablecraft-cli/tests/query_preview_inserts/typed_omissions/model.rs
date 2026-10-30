//! Complete batch images from qualified original host sources and literal rows.
use super::super::{PAGE, default_clock, omissions, support};
use tablecraft_core::values::{LegacyDateTime, Value};

const NOW_BITS: u64 = 0x40e1dd3000000000;
const SOURCE_ROWS: usize = 3;
const INDEX_START: usize = 480;
const INDEX_BITMAP: usize = 27;
const SHARED_PREFIX: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Integer,
    Clock,
}

#[derive(Clone)]
pub(super) struct Row {
    pub id: i32,
    pub score: i32,
    pub other: Option<u64>,
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub original: Vec<Row>,
    pub added: Vec<Row>,
    pub requests: Vec<u8>,
    pub generated: bool,
    pub kind: Kind,
}

impl Row {
    pub(super) fn values(&self, kind: Kind) -> Vec<Value> {
        let other = self.other.map_or(Value::Null, |value| match kind {
            Kind::Integer => Value::Int32(i32::try_from(value).unwrap()),
            Kind::Clock => Value::DateTime(LegacyDateTime::from_bits(value).unwrap()),
        });
        vec![Value::Int32(self.id), Value::Int32(self.score), other]
    }
}

pub(super) fn case(code: u8, generated: bool, kind: Kind, count: usize) -> Case {
    assert!(matches!(count, 2 | 3));
    let (before, original) = match kind {
        Kind::Integer => (
            omissions::three_field_source(code, generated),
            vec![
                Row {
                    id: 1,
                    score: 10,
                    other: Some(100),
                },
                Row {
                    id: 2,
                    score: 20,
                    other: Some(200),
                },
                Row {
                    id: 3,
                    score: 30,
                    other: Some(300),
                },
            ],
        ),
        Kind::Clock => (
            default_clock::source(code, generated, "Now()"),
            vec![
                Row {
                    id: 1,
                    score: 10,
                    other: Some(0),
                },
                Row {
                    id: 2,
                    score: 20,
                    other: None,
                },
                Row {
                    id: 3,
                    score: 30,
                    other: Some(0xbff4000000000000),
                },
            ],
        ),
    };
    assert_eq!(before[20], code);
    assert_eq!(read32(&before, 10 * PAGE + 16), 3);
    assert_eq!(read32(&before, 10 * PAGE + 67), 3);
    if generated {
        assert_eq!(read32(&before, 10 * PAGE + 20), 30);
    }

    let mut requests = String::new();
    let mut added = Vec::new();
    for ordinal in 0..count {
        let id = 4 + ordinal as i32;
        let score = if generated {
            31 + ordinal as i32
        } else {
            40 + ordinal as i32 * 10
        };
        requests.push_str(&format!("int32\t{id}\t"));
        if !generated {
            requests.push_str(&format!("int32\t{score}\t"));
        }
        let other = match (kind, ordinal) {
            (_, 1) => {
                requests.push_str("null\t\n");
                None
            }
            (Kind::Integer, 2) => {
                requests.push_str("int32\t77\n");
                Some(77)
            }
            (Kind::Integer, _) => {
                requests.push_str("omitted\t\n");
                Some(99)
            }
            (Kind::Clock, _) => {
                requests.push_str("omitted\t\n");
                Some(NOW_BITS)
            }
        };
        added.push(Row { id, score, other });
    }
    let expected = append(&before, generated, kind, &added);
    Case {
        before,
        expected,
        original,
        added,
        requests: requests.into_bytes(),
        generated,
        kind,
    }
}

fn read32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn wire(row: &Row, kind: Kind) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.score.to_le_bytes());
    match kind {
        Kind::Integer => bytes.extend(
            row.other
                .map_or(0_i32, |value| i32::try_from(value).unwrap())
                .to_le_bytes(),
        ),
        Kind::Clock => bytes.extend(row.other.unwrap_or(0).to_le_bytes()),
    }
    bytes.push(3 | if row.other.is_some() { 4 } else { 0 });
    assert_eq!(
        bytes.len(),
        match kind {
            Kind::Integer => 15,
            Kind::Clock => 19,
        }
    );
    bytes
}

pub(super) fn append(before: &[u8], generated: bool, kind: Kind, added: &[Row]) -> Vec<u8> {
    let mut after = before.to_vec();
    let data = 12 * PAGE;
    assert_eq!(&before[data + 12..data + 14], &[3, 0]);
    let mut stop = usize::from(u16::from_le_bytes(
        before[data + 18..data + 20].try_into().unwrap(),
    ));
    for (ordinal, row) in added.iter().enumerate() {
        let record = wire(row, kind);
        let start = stop - record.len();
        after[data + start..data + stop].copy_from_slice(&record);
        support::put16(
            &mut after,
            data + 14 + (SOURCE_ROWS + ordinal) * 2,
            start as u16,
        );
        stop = start;
    }
    let total = SOURCE_ROWS + added.len();
    support::put16(&mut after, data + 12, total as u16);
    support::put16(&mut after, data + 2, (stop - 14 - total * 2) as u16);
    support::put32(&mut after, 10 * PAGE + 16, total as u32);
    support::put32(&mut after, 10 * PAGE + 67, total as u32);
    if generated {
        support::put32(
            &mut after,
            10 * PAGE + 20,
            added.last().unwrap().score as u32,
        );
    }

    // This genuine single physical owner has one primary declaration and the original four
    // shared key bytes. New positive keys remain in that exact prefix family.
    let leaf = &mut after[13 * PAGE..14 * PAGE];
    assert_eq!(&leaf[24..26], &(SHARED_PREFIX as u16).to_le_bytes());
    assert_eq!(
        &leaf[INDEX_START..INDEX_START + SHARED_PREFIX],
        &[127, 128, 0, 0]
    );
    assert_eq!(
        &leaf[INDEX_START + 19 - 5..INDEX_START + 19],
        &[30, 0, 0, 12, 2]
    );
    let mut end = 19;
    leaf[INDEX_BITMAP..INDEX_START].fill(0);
    for entry in 0..total {
        let length = if entry == 0 { 9 } else { 9 + entry * 5 };
        leaf[INDEX_BITMAP + length / 8] |= 1 << (length % 8);
    }
    for (ordinal, row) in added.iter().enumerate() {
        let key = ((row.score as u32) ^ 0x80000000).to_be_bytes();
        assert_eq!(&key[..3], &[128, 0, 0]);
        let reference = ((12_u32 << 8) | (SOURCE_ROWS + ordinal) as u32).to_be_bytes();
        leaf[INDEX_START + end] = key[3];
        leaf[INDEX_START + end + 1..INDEX_START + end + 5].copy_from_slice(&reference);
        end += 5;
    }
    support::put16(leaf, 2, (PAGE - INDEX_START - end) as u16);

    for page in 0..before.len() / PAGE {
        if !matches!(page, 10 | 12 | 13) {
            assert_eq!(
                &after[page * PAGE..(page + 1) * PAGE],
                &before[page * PAGE..(page + 1) * PAGE]
            );
        }
    }
    let old_width = match kind {
        Kind::Integer => 15,
        Kind::Clock => 19,
    };
    for row in 0..SOURCE_ROWS {
        let at = data + 14 + row * 2;
        let offset = usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()));
        assert_eq!(
            &after[data + offset..data + offset + old_width],
            &before[data + offset..data + offset + old_width]
        );
    }
    after
}
