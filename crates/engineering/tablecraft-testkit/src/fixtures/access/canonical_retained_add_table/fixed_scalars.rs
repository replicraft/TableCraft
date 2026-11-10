//! Independent complete fixed-row measurements beside retained Access tables.
use super::desktop;
use crate::fixtures::access::{
    canonical_initial_table_creation::Column as NativeColumn,
    generated_record_insert::placement::{self, Pages},
    raw_forwarding_chain, raw_record_update,
};
use tablecraft_core::values::{Double, Single, Value};

const PAGE: usize = 4096;
const RECORD_BYTES: usize = 42;

const fn fixed(name: &'static str, native_type: u8, offset: u16, width: u16) -> NativeColumn {
    NativeColumn {
        name,
        native_type,
        fixed_offset: Some(offset),
        width,
        variable_index: 0,
        precision: 0,
        scale: 0,
    }
}

const COLUMNS: [NativeColumn; 8] = [
    fixed("ID", 4, 0, 4),
    fixed("Score", 4, 4, 4),
    fixed("ByteValue", 2, 8, 1),
    fixed("ShortValue", 3, 9, 2),
    fixed("LargeValue", 19, 11, 8),
    fixed("SingleValue", 6, 19, 4),
    fixed("DoubleValue", 7, 23, 8),
    fixed("DateValue", 8, 31, 8),
];

#[derive(Clone, Copy, Debug)]
pub enum Column {
    ByteValue,
    ShortValue,
    LargeValue,
    SingleValue,
    DoubleValue,
    DateValue,
}

pub const COLUMNS_TO_EDIT: [Column; 6] = [
    Column::ByteValue,
    Column::ShortValue,
    Column::LargeValue,
    Column::SingleValue,
    Column::DoubleValue,
    Column::DateValue,
];

impl Column {
    pub const fn number(self) -> usize {
        match self {
            Self::ByteValue => 2,
            Self::ShortValue => 3,
            Self::LargeValue => 4,
            Self::SingleValue => 5,
            Self::DoubleValue => 6,
            Self::DateValue => 7,
        }
    }

    pub fn name(self) -> &'static str {
        COLUMNS[self.number()].name
    }
}

/// Literal payload carriers, keeping floating and OLE date bits independent of
/// native writer codecs and of display/input conversion.
#[derive(Clone, Copy, Debug)]
pub enum Scalar {
    Byte(u8),
    Int16(i16),
    Int64(i64),
    SingleBits(u32),
    DoubleBits(u64),
    DateTimeBits(u64),
}

fn name(text: &str) -> Vec<u8> {
    let payload = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut bytes = (payload.len() as u16).to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((6 + payload.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn properties() -> Vec<u8> {
    let mut dictionary = name("Required");
    dictionary.extend(name("DefaultValue"));
    let mut body = b"MR2\0".to_vec();
    body.extend(block(128, &dictionary));
    for column in &COLUMNS[1..] {
        let label = name(column.name);
        let mut group = ((4 + label.len()) as u32).to_le_bytes().to_vec();
        group.extend(label);
        group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]); // Required=false.
        match column.native_type {
            // Typed IEEE defaults use the creator's exact finite scientific
            // spelling, including the exponent for positive floating zero.
            6 | 7 => group.extend([14, 0, 1, 12, 1, 0, 6, 0, b'0', 0, b'e', 0, b'0', 0]),
            8 => {}
            _ => group.extend([10, 0, 1, 12, 1, 0, 2, 0, b'0', 0]),
        }
        body.extend(block(1, &group));
    }
    body
}

/// Desktop ID/PrimaryKey, spare LONG Score and six nullable fixed scalars.
/// Every numeric companion defaults to zero; legacy DateTime has no default.
pub fn append_table(before: &[u8], table: &str) -> Vec<u8> {
    let template = desktop::initial::table(&COLUMNS, &properties());
    super::append_template(before, table, &template, Some(&properties()))
}

fn declaration(before: &[u8], definition: u32) {
    let page = &before[definition as usize * PAGE..(definition as usize + 1) * PAGE];
    assert_eq!(&page[41..47], &[8, 0, 0, 0, 8, 0]);
    for (number, column) in COLUMNS.iter().enumerate() {
        let at = 75 + number * 25;
        assert_eq!(page[at], column.native_type);
        assert_eq!(&page[at + 5..at + 7], &(number as u16).to_le_bytes());
        assert_eq!(page[at + 15], if number == 0 { 7 } else { 3 });
        assert_eq!(
            &page[at + 21..at + 23],
            &column.fixed_offset.unwrap().to_le_bytes()
        );
        assert_eq!(&page[at + 23..at + 25], &column.width.to_le_bytes());
    }
}

/// Append one resolved omitted row using real LAST and physical slot frontier.
pub fn append(before: &[u8], definition: u32) -> Vec<u8> {
    declaration(before, definition);
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = placement::next_id(before, pages);
    let mut record = vec![8, 0];
    record.extend(id.to_le_bytes());
    record.extend([0; 35]);
    record.push(0x7f); // All numeric fields present, DateValue NULL.
    assert_eq!(record.len(), RECORD_BYTES);
    placement::append(before, pages, id, &record)
}

/// Independent semantic outcome for an omitted row before any cell edit.
pub fn row(id: i32) -> Vec<Value> {
    vec![
        Value::Int32(id),
        Value::Int32(0),
        Value::Byte(0),
        Value::Int16(0),
        Value::Int64(0),
        Value::Single(Single::from_bits(0)),
        Value::Double(Double::from_bits(0)),
        Value::Null,
    ]
}

/// Change exactly the selected fixed payload and presence bit. NULL retains
/// ignored bytes, and matching values leave the complete source untouched.
pub fn update(
    before: &[u8],
    definition: u32,
    logical_root: usize,
    column: Column,
    value: Option<Scalar>,
) -> Vec<u8> {
    declaration(before, definition);
    let number = column.number();
    let selected = &COLUMNS[number];
    let range = raw_forwarding_chain::terminal_range(before, definition + 1, logical_root);
    let record = &before[range.clone()];
    assert_eq!(record.len(), RECORD_BYTES);
    assert_eq!(&record[..2], &[8, 0]);
    let mut payload = Vec::new();
    match (column, value) {
        (_, None) => {}
        (Column::ByteValue, Some(Scalar::Byte(value))) => payload.push(value),
        (Column::ShortValue, Some(Scalar::Int16(value))) => payload.extend(value.to_le_bytes()),
        (Column::LargeValue, Some(Scalar::Int64(value))) => payload.extend(value.to_le_bytes()),
        (Column::SingleValue, Some(Scalar::SingleBits(value))) => {
            payload.extend(value.to_le_bytes())
        }
        (Column::DoubleValue, Some(Scalar::DoubleBits(value)))
        | (Column::DateValue, Some(Scalar::DateTimeBits(value))) => {
            payload.extend(value.to_le_bytes());
        }
        _ => panic!("literal scalar must match the selected native declaration"),
    }
    let offset = 2 + usize::from(selected.fixed_offset.unwrap());
    let bit = 1 << number;
    let mut after = before.to_vec();
    let changed = &mut after[range];
    if value.is_some() {
        assert_eq!(payload.len(), usize::from(selected.width));
        changed[offset..offset + payload.len()].copy_from_slice(&payload);
        changed[RECORD_BYTES - 1] |= bit;
    } else {
        changed[RECORD_BYTES - 1] &= !bit;
    }
    after
}

/// Spare LONG action provides an old-family compatibility control on this table.
pub fn update_score(before: &[u8], definition: u32, root: usize, value: Option<i32>) -> Vec<u8> {
    declaration(before, definition);
    raw_record_update::number(
        before,
        definition + 1,
        root,
        definition as usize * PAGE + 100,
        1,
        value,
    )
}

/// Resolve a supplied scalar row independently by applying six exact fixed-cell
/// overlays to the literal fresh record. Only one append changes counters/keys.
pub fn append_values(before: &[u8], definition: u32, values: &[Scalar; 6]) -> Vec<u8> {
    let data = (definition as usize + 1) * PAGE;
    let root = usize::from(u16::from_le_bytes(
        before[data + 12..data + 14].try_into().unwrap(),
    ));
    let mut after = append(before, definition);
    for (column, value) in COLUMNS_TO_EDIT.into_iter().zip(values) {
        after = update(&after, definition, root, column, Some(*value));
    }
    after
}
