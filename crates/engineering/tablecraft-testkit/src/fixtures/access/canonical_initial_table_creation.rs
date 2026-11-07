//! Independently authored empty Increment schemas, without production codecs.
use super::{canonical_creation_primary, put16, put32};

/// Empty initial-table declarations with one generated LONG primary key.
#[derive(Clone, Copy, Debug)]
pub enum Case {
    IdOnly,
    ShortText,
    Mixed,
}

pub const CASES: [Case; 3] = [Case::IdOnly, Case::ShortText, Case::Mixed];

/// Literal native metadata, independent of CreateColumn and ScalarType.
pub struct Column {
    pub name: &'static str,
    pub native_type: u8,
    pub fixed_offset: Option<u16>,
    pub width: u16,
    pub variable_index: u16,
    pub precision: u8,
    pub scale: u8,
}

const fn fixed(name: &'static str, native_type: u8, offset: u16, width: u16) -> Column {
    Column {
        name,
        native_type,
        fixed_offset: Some(offset),
        width,
        variable_index: 0,
        precision: 0,
        scale: 0,
    }
}

const fn variable(name: &'static str, native_type: u8, width: u16, index: u16) -> Column {
    Column {
        name,
        native_type,
        fixed_offset: None,
        width,
        variable_index: index,
        precision: 0,
        scale: 0,
    }
}

const ID_ONLY: [Column; 1] = [fixed("Id", 4, 0, 4)];
const SHORT_TEXT: [Column; 2] = [fixed("Id", 4, 0, 4), variable("Name", 10, 510, 0)];
const MIXED: [Column; 14] = [
    fixed("Id", 4, 0, 4),
    fixed("Flag", 1, 0, 0),
    fixed("ByteValue", 2, 4, 1),
    fixed("ShortValue", 3, 5, 2),
    fixed("LongValue", 4, 7, 4),
    fixed("LargeValue", 19, 11, 8),
    fixed("Money", 5, 19, 8),
    fixed("SingleValue", 6, 27, 4),
    fixed("DoubleValue", 7, 31, 8),
    fixed("DateValue", 8, 39, 8),
    fixed("GuidValue", 15, 47, 16),
    variable("TextValue", 10, 40, 0),
    variable("BinaryValue", 9, 16, 1),
    Column {
        name: "DecimalValue",
        native_type: 16,
        fixed_offset: None,
        width: 17,
        variable_index: 2,
        precision: 18,
        scale: 4,
    },
];

pub struct Source {
    pub bytes: Vec<u8>,
    pub columns: &'static [Column],
}

/// Reuse only the independently authored empty envelope, map and primary leaf.
/// The complete user definition below is authored from literal metadata. No
/// production creator, schema decoder, native planner or SDK supplies the oracle.
pub fn source(case: Case) -> Source {
    let columns: &[Column] = match case {
        Case::IdOnly => &ID_ONLY,
        Case::ShortText => &SHORT_TEXT,
        Case::Mixed => &MIXED,
    };
    let mut bytes =
        canonical_creation_primary::source(canonical_creation_primary::Case::Empty).bytes;
    let original = &bytes[7 * 4096..8 * 4096];
    let old_length = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    // The unchanged two-field independent primary fixture has a 75-byte header,
    // two 25-byte declarations, then 6/12-byte UTF-16 name records.
    let old_index_start = 75 + 2 * 25 + 6 + 12;
    let mut definition = original[..75].to_vec();
    let variables = columns
        .iter()
        .filter(|column| column.fixed_offset.is_none())
        .count();
    put16(&mut definition, 41, columns.len() as u16);
    put16(&mut definition, 43, variables as u16);
    put16(&mut definition, 45, columns.len() as u16);
    assert_eq!(&definition[20..24], &[0; 4]);
    assert_eq!(&definition[47..55], &[1, 0, 0, 0, 1, 0, 0, 0]);

    for (position, column) in columns.iter().enumerate() {
        let mut raw = [0; 25];
        raw[0] = column.native_type;
        put32(&mut raw, 1, 1625);
        put16(&mut raw, 5, position as u16);
        put16(&mut raw, 7, column.variable_index);
        put16(&mut raw, 9, position as u16);
        if column.native_type == 10 {
            raw[11..15].copy_from_slice(&[9, 4, 0, 1]);
        } else if column.native_type == 16 {
            raw[11] = column.precision;
            raw[12] = column.scale;
        }
        raw[15] = if position == 0 {
            7
        } else if column.fixed_offset.is_some() {
            3
        } else {
            2
        };
        put16(&mut raw, 21, column.fixed_offset.unwrap_or(0));
        put16(&mut raw, 23, column.width);
        definition.extend(raw);
    }
    for column in columns {
        let units = column.name.encode_utf16().collect::<Vec<_>>();
        definition.extend((units.len() as u16 * 2).to_le_bytes());
        for unit in units {
            definition.extend(unit.to_le_bytes());
        }
    }
    definition.extend(&original[old_index_start..old_length]);
    let length = definition.len();
    put16(&mut definition, 2, (4096 - length - 8) as u16);
    put32(&mut definition, 8, length as u32);
    definition.resize(4096, 0);
    bytes[7 * 4096..8 * 4096].copy_from_slice(&definition);
    Source { bytes, columns }
}
