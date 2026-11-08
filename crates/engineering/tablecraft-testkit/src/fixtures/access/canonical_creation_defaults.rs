//! Literal creation-default sources; no creator, resolver or mutation imports.
use super::{
    canonical_creation as original, canonical_native17_creation as native17, data_page, put16,
};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub enum Case {
    Absent,
    Seven,
    Minimum,
    Null,
    RequiredNull,
    EmptySeven,
}

pub const CASES: [Case; 6] = [
    Case::Absent,
    Case::Seven,
    Case::Minimum,
    Case::Null,
    Case::RequiredNull,
    Case::EmptySeven,
];

impl Case {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Seven => "seven",
            Self::Minimum => "minimum",
            Self::Null => "null",
            Self::RequiredNull => "required-null",
            Self::EmptySeven => "empty-seven",
        }
    }
}

pub struct Source {
    pub bytes: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub expression: Option<&'static str>,
    pub required: Option<bool>,
    pub property_bytes: Option<Vec<u8>>,
    /// Independent expected omitted Score, including explicit stored NULL.
    pub omitted_score: Option<i32>,
    /// A healthy BEFORE whose future omission violates Required has no AFTER.
    pub omitted_refused: bool,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn name(text: &str) -> Vec<u8> {
    let payload = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut result = u16::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    result.extend(payload);
    result
}

fn block(kind: u16, body: &[u8]) -> Vec<u8> {
    let mut result = u32::try_from(body.len() + 6)
        .unwrap()
        .to_le_bytes()
        .to_vec();
    result.extend(kind.to_le_bytes());
    result.extend(body);
    result
}

fn property_bag(expression: Option<&str>, required: Option<bool>) -> Option<Vec<u8>> {
    if expression.is_none() && required.is_none() {
        return None;
    }
    let mut dictionary = Vec::new();
    if required.is_some() {
        dictionary.extend(name("Required"));
    }
    if expression.is_some() {
        dictionary.extend(name("DefaultValue"));
    }
    let mut group = 16_u32.to_le_bytes().to_vec();
    group.extend(name("Score"));
    if let Some(required) = required {
        group.extend(9_u16.to_le_bytes());
        group.extend([1, 1]);
        group.extend(0_u16.to_le_bytes());
        group.extend(1_u16.to_le_bytes());
        group.push(if required { 255 } else { 0 });
    }
    if let Some(expression) = expression {
        let payload = expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        group.extend(u16::try_from(8 + payload.len()).unwrap().to_le_bytes());
        group.extend([1, 12]); // DDL flag1, MEMO property type12, plain UTF16 payload.
        group.extend((if required.is_some() { 1_u16 } else { 0_u16 }).to_le_bytes());
        group.extend(u16::try_from(payload.len()).unwrap().to_le_bytes());
        group.extend(payload);
    }
    let mut result = b"MR2\0".to_vec();
    result.extend(block(128, &dictionary));
    result.extend(block(1, &group));
    Some(result)
}

fn attach(bytes: &mut [u8], bag: &[u8]) {
    let page = &bytes[4 * 4096..5 * 4096];
    assert_eq!(&page[..2], &[1, 1]);
    assert_eq!(word(page, 12), 8);
    let mut records = Vec::new();
    let mut end = 4096;
    for slot in 0..8 {
        let start = word(page, 14 + slot * 2);
        records.push(page[start..end].to_vec());
        end = start;
    }
    let old = &records[7];
    assert_eq!(&old[2..6], &7_u32.to_le_bytes());
    assert_eq!(word(old, 0), 17);
    assert_eq!(&old[old.len() - 3..], &[0xbf, 0, 0]);
    let footer = old.len() - 29;
    assert_eq!(word(old, footer), footer);
    assert_eq!(word(old, old.len() - 5), 11);
    assert_eq!(word(old, footer + 22), 32);
    for slot in 1..11 {
        assert_eq!(word(old, footer + 2 + 2 * (10 - slot)), footer);
    }
    let mut row = old[..footer].to_vec();
    row.extend((0x8000_0000_u32 | u32::try_from(bag.len()).unwrap()).to_le_bytes());
    row.extend([0; 8]);
    row.extend(bag);
    let eod = row.len();
    row.extend(u16::try_from(eod).unwrap().to_le_bytes());
    for slot in (0..11).rev() {
        let start = match slot {
            0 => 32,
            1..=8 => footer,
            _ => eod,
        };
        row.extend(u16::try_from(start).unwrap().to_le_bytes());
    }
    row.extend(11_u16.to_le_bytes());
    row.extend([0xbf, 0x40, 0]);
    assert_eq!(row.len(), old.len() + bag.len() + 12);
    records[7] = row;
    let used = 30 + records.iter().map(Vec::len).sum::<usize>();
    let records = records.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
    let mut page = data_page(2, &records);
    put16(&mut page, 2, u16::try_from(4096 - used).unwrap());
    bytes[4 * 4096..5 * 4096].copy_from_slice(&page);
}

pub fn source(case: Case) -> Source {
    let base = if matches!(case, Case::EmptySeven) {
        original::Case::Empty
    } else {
        original::Case::One
    };
    let original = original::source(base);
    let rows = original
        .rows
        .into_iter()
        .map(|row| row.into_iter().map(Value::Int32).collect())
        .collect();
    let mut bytes = native17::source(original.bytes);
    let expression = match case {
        Case::Absent => None,
        Case::Seven | Case::EmptySeven => Some("7"),
        Case::Minimum => Some("-2147483648"),
        Case::Null | Case::RequiredNull => Some("Null"),
    };
    let required = if matches!(case, Case::RequiredNull) {
        Some(true)
    } else {
        None
    };
    let property_bytes = property_bag(expression, required);
    if let Some(bag) = &property_bytes {
        attach(&mut bytes, bag);
    }
    Source {
        bytes,
        rows,
        expression,
        required,
        property_bytes,
        omitted_score: match case {
            Case::Seven | Case::EmptySeven => Some(7),
            Case::Minimum => Some(i32::MIN),
            _ => None,
        },
        omitted_refused: matches!(case, Case::RequiredNull),
    }
}

/// Literal append from original source bytes and declared request data only.
/// Supplied NULL does not consult, evaluate or replace the stored default.
pub fn inserted(source: &Source, supplied_null: bool) -> Option<(Vec<u8>, Vec<Vec<Value>>)> {
    if source.omitted_refused {
        return None;
    }
    let id = if supplied_null { 25_i32 } else { 24_i32 };
    let score = if supplied_null {
        None
    } else {
        source.omitted_score
    };
    let mut bytes = source.bytes.clone();
    let page = &mut bytes[8 * 4096..9 * 4096];
    let count = word(page, 12);
    assert_eq!(count, source.rows.len());
    let end = if count == 0 {
        4096
    } else {
        word(page, 14 + 2 * (count - 1))
    };
    let start = end - 11;
    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(id.to_le_bytes());
    row.extend(score.unwrap_or(0).to_le_bytes());
    row.push(if score.is_some() { 3 } else { 1 });
    page[start..end].copy_from_slice(&row);
    put16(page, 14 + 2 * count, u16::try_from(start).unwrap());
    put16(page, 12, u16::try_from(count + 1).unwrap());
    put16(
        page,
        2,
        u16::try_from(start - (14 + 2 * (count + 1))).unwrap(),
    );
    bytes[7 * 4096 + 16..7 * 4096 + 20]
        .copy_from_slice(&u32::try_from(count + 1).unwrap().to_le_bytes());
    let mut rows = source.rows.clone();
    rows.push(vec![
        Value::Int32(id),
        score.map_or(Value::Null, Value::Int32),
    ]);
    Some((bytes, rows))
}

/// Native17 ordinary Int32/DATE source with one literal Now() default.
/// This local mechanics fixture is separate from the source18 native/SDK corpus.
pub fn clock_source() -> Vec<u8> {
    let mut bytes = native17::source(original::source(original::Case::One).bytes);
    // Independent source definition: column 1 is a fixed eight-byte DATE at 4.
    bytes[7 * 4096 + 88] = 8;
    put16(&mut bytes, 7 * 4096 + 111, 8);
    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(1_i32.to_le_bytes());
    row.extend(0_f64.to_le_bytes());
    row.push(3);
    let mut page = data_page(7, &[(0, row)]);
    put16(&mut page, 2, 4065);
    bytes[8 * 4096..9 * 4096].copy_from_slice(&page);
    let bag = property_bag(Some("Now()"), None).unwrap();
    attach(&mut bytes, &bag);
    bytes
}

/// Independent literal append to the local two-column clock source.
/// The expected DATE bits are supplied as data, with no clock or native codec.
pub fn clock_inserted(before: &[u8], id: i32, date_bits: u64) -> Vec<u8> {
    clock_inserted_rows(before, &[(id, Some(date_bits))])
}

/// Independent ordered raw append, including explicit NULL and an empty batch.
/// Expected DATE bits are input data; no engine planner or default evaluator is used.
pub fn clock_inserted_rows(before: &[u8], rows: &[(i32, Option<u64>)]) -> Vec<u8> {
    let mut after = before.to_vec();
    for &(id, date_bits) in rows {
        let page = &mut after[8 * 4096..9 * 4096];
        let count = word(page, 12);
        let end = word(page, 14 + 2 * (count - 1));
        let start = end - 15;
        let mut row = 2_u16.to_le_bytes().to_vec();
        row.extend(id.to_le_bytes());
        row.extend(date_bits.unwrap_or(0).to_le_bytes());
        row.push(if date_bits.is_some() { 3 } else { 1 });
        page[start..end].copy_from_slice(&row);
        put16(page, 14 + 2 * count, u16::try_from(start).unwrap());
        put16(page, 12, u16::try_from(count + 1).unwrap());
        put16(
            page,
            2,
            u16::try_from(start - (14 + 2 * (count + 1))).unwrap(),
        );
        after[7 * 4096 + 16..7 * 4096 + 20]
            .copy_from_slice(&u32::try_from(count + 1).unwrap().to_le_bytes());
    }
    after
}

/// Ordinary native17 source whose 255 live DATA slots force a represented EOF.
/// Slot saturation is distinct from unused payload bytes; no native planner is used.
pub fn slot_saturated_source() -> Vec<u8> {
    let mut before = native17::source(original::source(original::Case::One).bytes);
    let rows = (1_i32..=255)
        .map(|id| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend((id * 10).to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    let mut page = data_page(7, &rows);
    put16(&mut page, 2, 767);
    before[8 * 4096..9 * 4096].copy_from_slice(&page);
    before[7 * 4096 + 16..7 * 4096 + 20].copy_from_slice(&255_u32.to_le_bytes());

    // Represented global free map page1/row0 grants only future page9.
    let global = word(&before[4096..2 * 4096], 14);
    before[4096 + global + 6] = 2;
    // Table candidate page3/row7 no longer offers slot-saturated page8.
    let candidate = word(&before[3 * 4096..4 * 4096], 14 + 2 * 7);
    before[3 * 4096 + candidate + 6] = 0;
    before
}

/// Independent literal EOF insertion of row (256,2560) into the saturated source.
pub fn slot_saturated_inserted(before: &[u8]) -> Vec<u8> {
    assert_eq!(before.len(), 9 * 4096);
    let mut after = before.to_vec();
    after[7 * 4096 + 16..7 * 4096 + 20].copy_from_slice(&256_u32.to_le_bytes());
    let global = word(&before[4096..2 * 4096], 14);
    after[4096 + global + 6] = 0;
    for slot in [3, 7] {
        let map = word(&before[3 * 4096..4 * 4096], 14 + 2 * slot);
        after[3 * 4096 + map + 6] |= 2;
    }
    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(256_i32.to_le_bytes());
    row.extend(2560_i32.to_le_bytes());
    row.push(3);
    let mut page = data_page(7, &[(0, row)]);
    put16(&mut page, 2, 4069);
    after.extend(page);
    after
}
