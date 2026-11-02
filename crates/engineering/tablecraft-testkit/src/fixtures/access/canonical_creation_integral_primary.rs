//! Authored manual wide-primary sources; no creator, mutation serializer or producer.
use super::{
    canonical_creation as original, canonical_native17_creation as native17, data_page, put16,
    put32,
};
use tablecraft_core::values::{Currency, Value};

#[derive(Clone, Copy)]
pub enum Family {
    Int64,
    Currency,
}

impl Family {
    pub const fn native_type(self) -> u8 {
        match self {
            Self::Int64 => 19,
            Self::Currency => 5,
        }
    }

    pub fn value(self, units: i64) -> Value {
        match self {
            Self::Int64 => Value::Int64(units),
            Self::Currency => Value::Currency(Currency::from_scaled(units)),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Scenario {
    Empty,
    SignedUnsorted,
    Duplicate,
    Null,
}

#[derive(Clone, Copy)]
pub struct Case {
    pub family: Family,
    pub scenario: Scenario,
}

pub const CASES: [Case; 8] = [
    Case {
        family: Family::Int64,
        scenario: Scenario::Empty,
    },
    Case {
        family: Family::Int64,
        scenario: Scenario::SignedUnsorted,
    },
    Case {
        family: Family::Int64,
        scenario: Scenario::Duplicate,
    },
    Case {
        family: Family::Int64,
        scenario: Scenario::Null,
    },
    Case {
        family: Family::Currency,
        scenario: Scenario::Empty,
    },
    Case {
        family: Family::Currency,
        scenario: Scenario::SignedUnsorted,
    },
    Case {
        family: Family::Currency,
        scenario: Scenario::Duplicate,
    },
    Case {
        family: Family::Currency,
        scenario: Scenario::Null,
    },
];

impl Case {
    pub const fn id(self) -> &'static str {
        match (self.family, self.scenario) {
            (Family::Int64, Scenario::Empty) => "int64-empty",
            (Family::Int64, Scenario::SignedUnsorted) => "int64-signed-unsorted",
            (Family::Int64, Scenario::Duplicate) => "int64-duplicate",
            (Family::Int64, Scenario::Null) => "int64-null",
            (Family::Currency, Scenario::Empty) => "currency-empty",
            (Family::Currency, Scenario::SignedUnsorted) => "currency-signed-unsorted",
            (Family::Currency, Scenario::Duplicate) => "currency-duplicate",
            (Family::Currency, Scenario::Null) => "currency-null",
        }
    }
}

#[derive(Clone, Copy)]
pub enum Violation {
    DuplicateKey,
    NullKey,
}

pub struct Source {
    pub family: Family,
    pub bytes: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub violation: Option<Violation>,
    pub history: u32,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn packed(owner: u32, records: &[Vec<u8>]) -> Vec<u8> {
    let used = 14 + 2 * records.len() + records.iter().map(Vec::len).sum::<usize>();
    assert!(used <= 4096);
    let rows = records
        .iter()
        .cloned()
        .map(|row| (0, row))
        .collect::<Vec<_>>();
    let mut page = data_page(owner, &rows);
    put16(&mut page, 2, u16::try_from(4096 - used).unwrap());
    page
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut end = 4096;
    for slot in 0..word(page, 12) {
        let start = word(page, 14 + 2 * slot);
        result.push(page[start..end].to_vec());
        end = start;
    }
    result
}

fn data_record(row: &[Value]) -> Vec<u8> {
    let [id, Value::Int32(score)] = row else {
        panic!("closed two-field source law")
    };
    let id = match id {
        Value::Int64(value) => Some(*value),
        Value::Currency(value) => Some(value.scaled()),
        Value::Null => None,
        _ => panic!("closed primary source carrier"),
    };
    let mut raw = 2_u16.to_le_bytes().to_vec();
    raw.extend(id.unwrap_or(i64::MIN).to_le_bytes());
    raw.extend(score.to_le_bytes());
    raw.push(if id.is_some() { 3 } else { 2 });
    raw
}

fn keys(rows: &[Vec<Value>]) -> Vec<(Option<i64>, u32)> {
    let mut entries = rows
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let value = match &row[0] {
                Value::Int64(value) => Some(*value),
                Value::Currency(value) => Some(value.scaled()),
                Value::Null => None,
                _ => panic!("closed source key"),
            };
            (value, (8 << 8) | u32::try_from(slot).unwrap())
        })
        .collect::<Vec<_>>();
    entries.sort_unstable();
    entries
}

fn leaf(rows: &[Vec<Value>]) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[4, 1]);
    put32(&mut page, 4, 7);
    let mut active = 0;
    for (value, reference) in keys(rows) {
        let mut raw = if let Some(value) = value {
            let mut raw = vec![127];
            raw.extend(((value as u64) ^ 0x8000_0000_0000_0000).to_be_bytes());
            raw
        } else {
            vec![0]
        };
        raw.extend(reference.to_be_bytes());
        page[480 + active..480 + active + raw.len()].copy_from_slice(&raw);
        active += raw.len();
        page[27 + active / 8] |= 1 << (active % 8);
    }
    put16(&mut page, 2, u16::try_from(3616 - active).unwrap());
    page
}

fn definition(before: &[u8], family: Family, count: usize, history: u32) -> Vec<u8> {
    let length = u32::from_le_bytes(before[8..12].try_into().unwrap()) as usize;
    assert_eq!(&before[47..55], &[0; 8]);
    assert_eq!(&before[length - 2..length], &[255, 255]);
    let mut raw = before[..63].to_vec();
    raw.extend([0; 12]);
    put32(&mut raw, 67, history);
    raw.extend(&before[63..length - 2]);
    // Original independent Id/Score names and descriptor fields remain exact;
    // only the literal declared key family/width and Score offset differ.
    assert_eq!(raw[75], 4);
    assert_eq!(&raw[96..100], &[0, 0, 4, 0]);
    assert_eq!(&raw[121..125], &[4, 0, 4, 0]);
    raw[75] = family.native_type();
    put16(&mut raw, 98, 8);
    put16(&mut raw, 121, 8);
    let mut physical = vec![0; 52];
    put32(&mut physical, 0, 1923);
    for key in 0..10 {
        put16(&mut physical, 4 + 3 * key, 65535);
    }
    put16(&mut physical, 4, 0);
    physical[6] = 1;
    put32(&mut physical, 34, (3 << 8) | 22);
    put32(&mut physical, 38, 9);
    physical[46] = 137;
    raw.extend(physical);
    let mut logical = vec![0; 28];
    put32(&mut logical, 0, 1625);
    put32(&mut logical, 13, u32::MAX);
    logical[23] = 1;
    raw.extend(logical);
    let name = "PrimaryId"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    raw.extend(u16::try_from(name.len()).unwrap().to_le_bytes());
    raw.extend(name);
    raw.extend([255, 255]);
    let end = raw.len();
    put16(&mut raw, 2, u16::try_from(4096 - end - 8).unwrap());
    put32(&mut raw, 8, u32::try_from(end).unwrap());
    put32(&mut raw, 16, u32::try_from(count).unwrap());
    put32(&mut raw, 47, 1);
    put32(&mut raw, 51, 1);
    raw.resize(4096, 0);
    raw
}

pub fn source(case: Case) -> Source {
    let base = original::source(original::Case::Empty);
    let mut bytes = native17::source(base.bytes);
    let units: Vec<(Option<i64>, i32)> = match case.scenario {
        Scenario::Empty => Vec::new(),
        Scenario::SignedUnsorted => vec![
            (Some(i64::MAX), 40),
            (Some(i64::MIN), 10),
            (Some(0), 30),
            (Some(-1), 20),
        ],
        Scenario::Duplicate => vec![(Some(i64::MIN), 10), (Some(i64::MIN), 20)],
        Scenario::Null => vec![(None, 10), (Some(i64::MAX), 20)],
    };
    let rows = units
        .into_iter()
        .map(|(key, score)| {
            vec![
                key.map_or(Value::Null, |units| case.family.value(units)),
                Value::Int32(score),
            ]
        })
        .collect::<Vec<_>>();
    let entries = keys(&rows);
    let mut previous: Option<Option<i64>> = None;
    let mut history = 0_u32;
    for (value, _) in entries {
        if previous != Some(value) {
            history += 1;
        }
        previous = Some(value);
    }
    let table = definition(&bytes[7 * 4096..8 * 4096], case.family, rows.len(), history);
    bytes[7 * 4096..8 * 4096].copy_from_slice(&table);
    let data = packed(
        7,
        &rows.iter().map(|row| data_record(row)).collect::<Vec<_>>(),
    );
    bytes[8 * 4096..9 * 4096].copy_from_slice(&data);
    let mut maps = records(&bytes[3 * 4096..4 * 4096]);
    assert_eq!(maps.len(), 22);
    assert!(maps.iter().all(|map| map.len() == 13 && map[..5] == [0; 5]));
    let mut index_map = vec![0; 13];
    index_map[6] = 2;
    maps.push(index_map);
    bytes[3 * 4096..4 * 4096].copy_from_slice(&packed(0, &maps));
    assert_eq!(bytes.len(), 9 * 4096);
    bytes.extend(leaf(&rows));
    Source {
        family: case.family,
        bytes,
        rows,
        violation: match case.scenario {
            Scenario::Duplicate => Some(Violation::DuplicateKey),
            Scenario::Null => Some(Violation::NullKey),
            _ => None,
        },
        history,
    }
}

/// An independently literal normal INSERT witness, never a creation input fill.
pub fn inserted(source: &Source) -> Option<(Vec<u8>, Vec<Vec<Value>>)> {
    if source.violation.is_some() {
        return None;
    }
    let mut rows = source.rows.clone();
    rows.push(vec![source.family.value(10001), Value::Int32(17)]);
    let mut bytes = source.bytes.clone();
    let page = &mut bytes[8 * 4096..9 * 4096];
    let count = word(page, 12);
    assert_eq!(count, source.rows.len());
    let end = if count == 0 {
        4096
    } else {
        word(page, 14 + 2 * (count - 1))
    };
    let start = end - 15;
    page[start..end].copy_from_slice(&data_record(rows.last().unwrap()));
    put16(page, 14 + 2 * count, u16::try_from(start).unwrap());
    put16(page, 12, u16::try_from(count + 1).unwrap());
    put16(
        page,
        2,
        u16::try_from(start - 14 - 2 * (count + 1)).unwrap(),
    );
    put32(&mut bytes, 7 * 4096 + 16, u32::try_from(count + 1).unwrap());
    put32(&mut bytes, 7 * 4096 + 67, source.history + 1);
    bytes[9 * 4096..10 * 4096].copy_from_slice(&leaf(&rows));
    Some((bytes, rows))
}

/// Literal removal of the just-inserted logical row. No mutation helper is used.
pub fn deleted_after_insert(source: &Source) -> Option<(Vec<u8>, Vec<Vec<Value>>)> {
    let (mut bytes, inserted_rows) = inserted(source)?;
    let slot = source.rows.len();
    assert_eq!(inserted_rows.len(), slot + 1);
    let at = 8 * 4096 + 14 + 2 * slot;
    let original = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(original & 0xf000, 0);
    put16(&mut bytes, at, original | 0xc000);
    put32(&mut bytes, 7 * 4096 + 16, u32::try_from(slot).unwrap());

    // Retain every inserted tail byte beyond the new active end. The fresh
    // original-row leaf is only an independent source of the active overlay.
    let survivor_leaf = leaf(&source.rows);
    let active = 3616 - word(&survivor_leaf, 2);
    let page = &mut bytes[9 * 4096..10 * 4096];
    page[2..4].copy_from_slice(&survivor_leaf[2..4]);
    page[27..480 + active].copy_from_slice(&survivor_leaf[27..480 + active]);
    Some((bytes, source.rows.clone()))
}
