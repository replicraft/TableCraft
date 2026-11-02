//! Source-owned AutoNumber declarations and literal Increment row/counter laws.
//! No mode detector, creator, expression evaluator or mutation encoder is called.
use super::{canonical_creation_autonumber as original, data_page, put16, put32};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub enum Case {
    IncrementHigh,
    IncrementHighInserted,
    IncrementZero,
    IncrementZeroInserted,
    RandomDefault,
    UnknownDefault,
    IncrementMaximum,
}

pub const CASES: [Case; 7] = [
    Case::IncrementHigh,
    Case::IncrementHighInserted,
    Case::IncrementZero,
    Case::IncrementZeroInserted,
    Case::RandomDefault,
    Case::UnknownDefault,
    Case::IncrementMaximum,
];

impl Case {
    pub const fn id(self) -> &'static str {
        match self {
            Self::IncrementHigh => "increment-high-before",
            Self::IncrementHighInserted => "increment-high-inserted",
            Self::IncrementZero => "increment-zero-before",
            Self::IncrementZeroInserted => "increment-zero-inserted",
            Self::RandomDefault => "random-special-default",
            Self::UnknownDefault => "unknown-generator-default",
            Self::IncrementMaximum => "increment-maximum",
        }
    }
}

pub struct Source {
    pub bytes: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub last: i32,
    pub expression: Option<&'static str>,
    pub inserted: bool,
}

fn named(value: &str) -> Vec<u8> {
    let bytes = value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut result = (bytes.len() as u16).to_le_bytes().to_vec();
    result.extend(bytes);
    result
}

fn block(kind: u16, body: &[u8]) -> Vec<u8> {
    let mut result = (6_u32 + body.len() as u32).to_le_bytes().to_vec();
    result.extend(kind.to_le_bytes());
    result.extend(body);
    result
}

fn properties(expression: &str) -> Vec<u8> {
    let name = named("Id");
    let mut group = (4_u32 + name.len() as u32).to_le_bytes().to_vec();
    group.extend(name);
    let payload = expression
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    group.extend((8_u16 + payload.len() as u16).to_le_bytes());
    group.extend([1, 12]);
    group.extend(0_u16.to_le_bytes());
    group.extend((payload.len() as u16).to_le_bytes());
    group.extend(payload);

    let mut result = b"MR2\0".to_vec();
    result.extend(block(128, &named("DefaultValue")));
    result.extend(block(1, &group));
    assert_eq!(result.len(), 60 + 2 * expression.len());
    result
}

fn attach_properties(bytes: &mut [u8], expression: &str) {
    let page = &bytes[4 * 4096..5 * 4096];
    assert_eq!(&page[..2], &[1, 1]);
    assert_eq!(u16::from_le_bytes(page[12..14].try_into().unwrap()), 8);
    let mut records = Vec::new();
    let mut end = 4096;
    for slot in 0..8 {
        let at = 14 + 2 * slot;
        let start = u16::from_le_bytes(page[at..at + 2].try_into().unwrap()) as usize;
        records.push(page[start..end].to_vec());
        end = start;
    }

    let old = &records[7];
    assert_eq!(&old[..2], &17_u16.to_le_bytes());
    assert_eq!(&old[2..6], &7_u32.to_le_bytes());
    assert_eq!(&old[old.len() - 3..], &[0xbf, 0, 0]);
    let footer = old.len() - 29;
    assert_eq!(&old[footer..footer + 2], &(footer as u16).to_le_bytes());
    assert_eq!(&old[old.len() - 5..old.len() - 3], &11_u16.to_le_bytes());
    for variable in 1..11 {
        let at = footer + 2 + 2 * (10 - variable);
        assert_eq!(&old[at..at + 2], &(footer as u16).to_le_bytes());
    }

    let bag = properties(expression);
    let mut row = old[..footer].to_vec();
    row.extend((0x8000_0000_u32 | bag.len() as u32).to_le_bytes());
    row.extend([0; 8]);
    row.extend(bag);
    let eod = row.len() as u16;
    row.extend(eod.to_le_bytes());
    for variable in (0..11).rev() {
        let start = if variable == 0 {
            32
        } else if variable <= 8 {
            footer as u16
        } else {
            eod
        };
        row.extend(start.to_le_bytes());
    }
    row.extend(11_u16.to_le_bytes());
    row.extend([0xbf, 0x40, 0]);
    records[7] = row;
    let records = records.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
    let mut catalog = data_page(2, &records);
    let used = 30 + records.iter().map(|(_, row)| row.len()).sum::<usize>();
    put16(&mut catalog, 2, (4096 - used) as u16);
    bytes[4 * 4096..5 * 4096].copy_from_slice(&catalog);
}

fn append(bytes: &mut [u8], identity: i32, original_rows: usize) {
    let start = 4096 - 11 * (original_rows + 1);
    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(identity.to_le_bytes());
    row.extend(17_i32.to_le_bytes());
    row.push(3);
    assert_eq!(row.len(), 11);
    let data = &mut bytes[8 * 4096..9 * 4096];
    data[start..start + 11].copy_from_slice(&row);
    put16(data, 14 + 2 * original_rows, start as u16);
    put16(data, 12, (original_rows + 1) as u16);
    put16(data, 2, (start - 14 - 2 * (original_rows + 1)) as u16);

    let leaf = &mut bytes[9 * 4096..10 * 4096];
    let mut wire = vec![127];
    wire.extend(((identity as u32) ^ 0x8000_0000).to_be_bytes());
    wire.extend(((8_u32 << 8) | original_rows as u32).to_be_bytes());
    let active = 9 * (original_rows + 1);
    leaf[480 + 9 * original_rows..480 + active].copy_from_slice(&wire);
    leaf[27 + active / 8] |= 1 << (active % 8);
    put16(leaf, 2, (3616 - active) as u16);
    put32(bytes, 7 * 4096 + 16, (original_rows + 1) as u32);
    put32(bytes, 7 * 4096 + 20, identity as u32);
    put32(bytes, 7 * 4096 + 67, (original_rows + 1) as u32);
}

pub fn source(case: Case) -> Source {
    let zero = matches!(case, Case::IncrementZero | Case::IncrementZeroInserted);
    let maximum = matches!(case, Case::IncrementMaximum);
    let base = if zero {
        original::Case::EmptyPrimary
    } else if maximum {
        original::Case::MaximumEmpty
    } else {
        original::Case::HighLast
    };
    let original = original::source(base);
    assert_eq!(original.bytes.len(), 10 * 4096);
    let mut bytes = original.bytes;
    let mut rows = original.rows;
    let expression = match case {
        Case::RandomDefault => Some("GenUniqueID()"),
        Case::UnknownDefault => Some("UnqualifiedGenerator()"),
        _ => None,
    };
    if let Some(expression) = expression {
        attach_properties(&mut bytes, expression);
    }

    let inserted = matches!(
        case,
        Case::IncrementHighInserted | Case::IncrementZeroInserted
    );
    let last = if inserted {
        let identity = if zero { 1 } else { 138 };
        append(&mut bytes, identity, rows.len());
        rows.push(vec![Value::Int32(identity), Value::Int32(17)]);
        identity
    } else {
        original.last
    };
    Source {
        bytes,
        rows,
        last,
        expression,
        inserted,
    }
}
