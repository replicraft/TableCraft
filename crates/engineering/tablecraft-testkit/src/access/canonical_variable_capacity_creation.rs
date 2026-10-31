//! Independent near-full variable records and candidate-map boundary SOURCEs.
use super::{canonical_variable_creation, data_page, put16, put32};
use tablecraft_core::values::{Decimal, Value};

const PAGE: usize = 4096;
const MAXIMUM_DECIMAL: i128 = 9_999_999_999_999_999_999_999_999_999;
const ESCAPED_TEXT: [u8; 19] = [
    0xff, 0xfe, 0, 0xff, 0xfe, b'a', 0, b'b', 0, b'c', 0, b'd', 0, b'e', 0, b'f', 0, b'g', 0,
];
const DECIMAL_BYTES: [u8; 17] = [
    0, 0, 0, 0, 0, 0x5e, 0xce, 0x4f, 0x20, 0x61, 0x02, 0x25, 0x3e, 0xff, 0xff, 0xff, 0x0f,
];

/// Two original SOURCEs differ by one actual minimum-sized legal row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Fifty-seven maximum encoded records leave room for a legal NULL row.
    MaximumRows,
    /// The same fifty-seven records plus one minimum NULL row exhaust that room.
    NullTail,
}

/// Frozen qualification order, separate from the original five variable cases.
pub const CASES: [Case; 2] = [Case::MaximumRows, Case::NullTail];

impl Case {
    /// Stable case identity, without deriving names from producer behavior.
    pub const fn id(self) -> &'static str {
        match self {
            Self::MaximumRows => "maximum-rows",
            Self::NullTail => "null-tail",
        }
    }
}

/// Original bytes, exact row facts and literal DATA/map expectations.
pub struct Source {
    /// Reused independent schema and complete original image.
    pub source: canonical_variable_creation::Source,
    /// Exact remaining DATA free bytes after the authored records/directory.
    pub free_bytes: u16,
    /// Whether another legal minimum record and directory slot still fit.
    pub candidate: bool,
}

/// Derive from the unchanged empty original; no creator, reader or codec runs.
pub fn source(case: Case) -> Source {
    let mut source = canonical_variable_creation::source(canonical_variable_creation::Case::Empty);
    let mut records = Vec::new();
    let mut rows = Vec::new();
    for id in 1_i32..=57 {
        let flag = id % 2 == 0;
        let mut record = vec![0; 6];
        put16(&mut record, 0, 5);
        record[2..6].copy_from_slice(&id.to_le_bytes());
        record.extend_from_slice(&ESCAPED_TEXT);
        record.extend_from_slice(&[0xff; 16]);
        record.extend_from_slice(&DECIMAL_BYTES);
        // EOD58, then starts41/25/6 in reverse, followed by count3 and mask.
        record.extend_from_slice(&[58, 0, 41, 0, 25, 0, 6, 0, 3, 0]);
        record.push(if flag { 0x1f } else { 0x0f });
        assert_eq!(record.len(), 69);
        records.push((0, record));
        rows.push(vec![
            Value::Int32(id),
            Value::Text("\u{feff}abcdefg".into()),
            Value::Binary(vec![0xff; 16]),
            Value::Decimal(Decimal::new(MAXIMUM_DECIMAL, 4).unwrap()),
            Value::Boolean(flag),
        ]);
    }
    if case == Case::NullTail {
        // No Required declarations exist. Id and all variable carriers are NULL;
        // Boolean false is represented by its clear Boolean mask bit.
        records.push((0, vec![5, 0, 0, 0, 0, 0, 6, 0, 6, 0, 6, 0, 6, 0, 3, 0, 0]));
        rows.push(vec![
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Boolean(false),
        ]);
    }
    let free_bytes = if case == Case::MaximumRows { 35 } else { 16 };
    let candidate = case == Case::MaximumRows;
    let mut data = data_page(7, &records);
    put16(&mut data, 2, free_bytes);
    source.bytes[8 * PAGE..9 * PAGE].copy_from_slice(&data);
    put32(&mut source.bytes, 7 * PAGE + 16, rows.len() as u32);
    if !candidate {
        let map = &mut source.bytes[3 * PAGE..4 * PAGE];
        let candidate_start = u16::from_le_bytes(map[28..30].try_into().unwrap()) as usize;
        assert_eq!(map[candidate_start + 6], 1);
        map[candidate_start + 6] = 0;
    }
    source.rows = rows;
    Source {
        source,
        free_bytes,
        candidate,
    }
}
