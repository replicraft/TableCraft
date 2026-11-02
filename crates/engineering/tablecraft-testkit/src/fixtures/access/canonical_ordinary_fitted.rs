//! Independent ordinary fitting sources and literal full-image outcomes.
//! No production mutation, scalar codec or decoder is imported.
use super::canonical_creation;
use tablecraft_core::values::{Currency, Decimal, LegacyDateTime, Value};

const PAGE: usize = 4096;
const FIXED_END: usize = 30;
const FOOTER: usize = 11;
const WORD: [u8; 17] = [0, 0, 0, 0, 0, 4, 3, 2, 1, 8, 7, 6, 5, 12, 11, 10, 9];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    NullTextToEmpty,
    EmptyTextToNull,
    NullBinaryToEmpty,
    EmptyBinaryToNull,
    CompressedText,
    RawUnicode,
    EscapedBom,
    BinaryGrowth,
    SharedExactFit,
    SharedReversed,
    JointFiveFixed,
    DisjointRows,
    ExactNoops,
    NullNoops,
    EmptyGroups,
    TextOverflow,
    SharedOverflow,
    StaleText,
    StaleBinary,
}

pub const RECIPES: [Recipe; 19] = [
    Recipe::NullTextToEmpty,
    Recipe::EmptyTextToNull,
    Recipe::NullBinaryToEmpty,
    Recipe::EmptyBinaryToNull,
    Recipe::CompressedText,
    Recipe::RawUnicode,
    Recipe::EscapedBom,
    Recipe::BinaryGrowth,
    Recipe::SharedExactFit,
    Recipe::SharedReversed,
    Recipe::JointFiveFixed,
    Recipe::DisjointRows,
    Recipe::ExactNoops,
    Recipe::NullNoops,
    Recipe::EmptyGroups,
    Recipe::TextOverflow,
    Recipe::SharedOverflow,
    Recipe::StaleText,
    Recipe::StaleBinary,
];

impl Recipe {
    pub const fn id(self) -> &'static str {
        match self {
            Self::NullTextToEmpty => "null-text-to-empty",
            Self::EmptyTextToNull => "empty-text-to-null",
            Self::NullBinaryToEmpty => "null-binary-to-empty",
            Self::EmptyBinaryToNull => "empty-binary-to-null",
            Self::CompressedText => "compressed-text",
            Self::RawUnicode => "raw-unicode",
            Self::EscapedBom => "escaped-bom",
            Self::BinaryGrowth => "binary-growth",
            Self::SharedExactFit => "shared-exact-fit",
            Self::SharedReversed => "shared-reversed",
            Self::JointFiveFixed => "joint-five-fixed",
            Self::DisjointRows => "disjoint-rows",
            Self::ExactNoops => "exact-noops",
            Self::NullNoops => "null-noops",
            Self::EmptyGroups => "empty-groups",
            Self::TextOverflow => "text-overflow",
            Self::SharedOverflow => "shared-overflow",
            Self::StaleText => "stale-text",
            Self::StaleBinary => "stale-binary",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Capacity,
    Conflict,
}

pub struct Update {
    pub row: usize,
    pub expected: Value,
    pub replacement: Value,
    /// Independently declared physical replacement; fixed values use their own law.
    pub payload: Option<&'static [u8]>,
}

pub struct Group {
    pub column: &'static str,
    pub number: usize,
    pub updates: Vec<Update>,
}

pub struct Frame {
    pub absolute: usize,
    pub length: usize,
    pub starts: [usize; 4],
    pub data_end: usize,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
    pub groups: Vec<Group>,
    pub frames: Vec<Frame>,
    pub changed: usize,
    pub refusal: Option<Refusal>,
}

fn put16(bytes: &mut [u8], at: usize, value: usize) {
    bytes[at..at + 2].copy_from_slice(&u16::try_from(value).unwrap().to_le_bytes());
}

fn put32(bytes: &mut [u8], at: usize, value: usize) {
    bytes[at..at + 4].copy_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}
fn binary(value: &[u8]) -> Value {
    Value::Binary(value.into())
}
fn currency(value: i64) -> Value {
    Value::Currency(Currency::from_scaled(value))
}
fn date(bits: u64) -> Value {
    Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
}

fn rows() -> Vec<Vec<Value>> {
    vec![
        vec![
            Value::Int32(1),
            text("alpha"),
            binary(&[0, 255, 1, 2]),
            Value::Decimal(Decimal::new(311_917_102_708_983_781_990_730_508, 4).unwrap()),
            Value::Boolean(false),
            Value::Int64(1000),
            currency(24690),
            date(0x3ff4000000000000),
        ],
        vec![
            Value::Int32(2),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Boolean(true),
            Value::Null,
            Value::Null,
            Value::Null,
        ],
        vec![
            Value::Int32(3),
            text(""),
            binary(&[]),
            Value::Null,
            Value::Boolean(false),
            Value::Int64(-7),
            currency(-10001),
            date(0x8000000000000000),
        ],
    ]
}

// SDK ordinary-first slot law: Text0/Binary1/Decimal2; subsequent fixed fields
// retain current unused variable index3. Boolean fixed offset is zero.
fn definition() -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[2, 1]);
    put32(&mut page, 12, 1625);
    put32(&mut page, 16, 3);
    page[24] = 1;
    page[40] = 0x4e;
    put16(&mut page, 41, 8);
    put16(&mut page, 43, 3);
    put16(&mut page, 45, 8);
    put32(&mut page, 55, (3 << 8) | 3);
    put32(&mut page, 59, (3 << 8) | 7);
    let columns = [
        ("Id", 4, 0, 0, 4, true),
        ("TextValue", 10, 0, 0, 16, false),
        ("BinaryValue", 9, 1, 0, 16, false),
        ("DecimalValue", 16, 2, 0, 17, false),
        ("Flag", 1, 3, 0, 0, true),
        ("Large", 19, 3, 4, 8, true),
        ("Money", 5, 3, 12, 8, true),
        ("Date", 8, 3, 20, 8, true),
    ];
    let mut names = 63 + 25 * 8;
    for (number, (name, native, slot, offset, width, fixed)) in columns.into_iter().enumerate() {
        let at = 63 + number * 25;
        page[at] = native;
        put32(&mut page, at + 1, 1625);
        put16(&mut page, at + 5, number);
        put16(&mut page, at + 7, slot);
        put16(&mut page, at + 9, number);
        if native == 10 {
            page[at + 11..at + 15].copy_from_slice(&[9, 4, 0, 1]);
            // Declare Unicode compression explicitly, matching the literal
            // compressed beta and aggregate shared-capacity expectations.
            page[at + 16] = 1;
        } else if native == 16 {
            page[at + 11] = 28;
            page[at + 12] = 4;
        }
        page[at + 15] = if fixed { 3 } else { 2 };
        put16(&mut page, at + 21, offset);
        put16(&mut page, at + 23, width);
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put16(&mut page, names, encoded.len());
        names += 2;
        page[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    put16(&mut page, names, 0xffff);
    put32(&mut page, 8, names + 2);
    put16(&mut page, 2, PAGE - names - 10);
    page
}

fn original_record(row: usize) -> (Vec<u8>, [usize; 4]) {
    let mut record = vec![0; FIXED_END];
    put16(&mut record, 0, 8);
    record[2..6].copy_from_slice(&((row + 1) as i32).to_le_bytes());
    match row {
        0 => {
            record[6..14].copy_from_slice(&1000_i64.to_le_bytes());
            record[14..22].copy_from_slice(&24690_i64.to_le_bytes());
            record[22..30].copy_from_slice(&0x3ff4000000000000_u64.to_le_bytes());
        }
        1 => {
            record[6..14].fill(0xa7);
            record[14..22].fill(0xce);
            record[22..30].fill(0xd0);
        }
        2 => {
            record[6..14].copy_from_slice(&(-7_i64).to_le_bytes());
            record[14..22].copy_from_slice(&(-10001_i64).to_le_bytes());
            record[22..30].copy_from_slice(&0x8000000000000000_u64.to_le_bytes());
        }
        _ => unreachable!(),
    }
    let payloads: [&[u8]; 3] = match row {
        0 => [b"a\0l\0p\0h\0a\0", &[0, 255, 1, 2], &WORD],
        1 => [&[], &[], &[]],
        2 => [&[255, 254], &[], &[]],
        _ => unreachable!(),
    };
    let mut starts = [0; 4];
    for (slot, payload) in payloads.into_iter().enumerate() {
        starts[slot] = record.len();
        record.extend_from_slice(payload);
    }
    starts[3] = record.len();
    record.resize(record.len() + [8, 12, 0][row], 0xa5);
    record.extend_from_slice(&(starts[3] as u16).to_le_bytes());
    for start in starts[..3].iter().rev() {
        record.extend_from_slice(&(*start as u16).to_le_bytes());
    }
    record.extend_from_slice(&3_u16.to_le_bytes());
    record.push([0xef, 0x11, 0xe7][row]);
    (record, starts)
}

fn group(
    number: usize,
    row: usize,
    expected: Value,
    replacement: Value,
    payload: Option<&'static [u8]>,
) -> Group {
    Group {
        column: [
            "Id",
            "TextValue",
            "BinaryValue",
            "DecimalValue",
            "Flag",
            "Large",
            "Money",
            "Date",
        ][number],
        number,
        updates: vec![Update {
            row,
            expected,
            replacement,
            payload,
        }],
    }
}

fn requests(recipe: Recipe) -> (Vec<Group>, Option<Refusal>) {
    let t = |row, old, new, raw| group(1, row, old, new, Some(raw));
    let b = |row, old, new, raw| group(2, row, old, new, Some(raw));
    let alpha = || text("alpha");
    let old_binary = || binary(&[0, 255, 1, 2]);
    let beta = || {
        t(
            0,
            alpha(),
            text("beta"),
            &[255, 254, b'b', b'e', b't', b'a'],
        )
    };
    let grow_binary = || b(0, old_binary(), binary(&[0xff; 16]), &[0xff; 16]);
    let groups = match recipe {
        Recipe::NullTextToEmpty => vec![t(1, Value::Null, text(""), &[255, 254])],
        Recipe::EmptyTextToNull => vec![t(2, text(""), Value::Null, &[])],
        Recipe::NullBinaryToEmpty => vec![b(1, Value::Null, binary(&[]), &[])],
        Recipe::EmptyBinaryToNull => vec![b(2, binary(&[]), Value::Null, &[])],
        Recipe::CompressedText => vec![beta()],
        Recipe::RawUnicode => vec![t(
            0,
            alpha(),
            text("Ω雪🙂a"),
            &[169, 3, 234, 150, 61, 216, 66, 222, 97, 0],
        )],
        Recipe::EscapedBom => vec![t(0, alpha(), text("\u{feff}"), &[255, 254, 0, 255, 254])],
        Recipe::BinaryGrowth => vec![b(0, old_binary(), binary(&[0xff; 12]), &[0xff; 12])],
        Recipe::SharedExactFit => vec![beta(), grow_binary()],
        Recipe::SharedReversed => vec![grow_binary(), beta()],
        Recipe::JointFiveFixed => vec![
            beta(),
            grow_binary(),
            group(0, 0, Value::Int32(1), Value::Int32(17), None),
            group(4, 0, Value::Boolean(false), Value::Boolean(true), None),
            group(5, 0, Value::Int64(1000), Value::Int64(-7), None),
            group(6, 0, currency(24690), Value::Null, None),
            group(
                7,
                0,
                date(0x3ff4000000000000),
                date(0x8000000000000000),
                None,
            ),
        ],
        Recipe::DisjointRows => vec![beta(), b(1, Value::Null, binary(&[0, 255]), &[0, 255])],
        Recipe::ExactNoops => vec![
            t(0, alpha(), alpha(), b"a\0l\0p\0h\0a\0"),
            b(0, old_binary(), old_binary(), &[0, 255, 1, 2]),
        ],
        Recipe::NullNoops => vec![
            t(1, Value::Null, Value::Null, &[]),
            b(1, Value::Null, Value::Null, &[]),
        ],
        Recipe::EmptyGroups => vec![
            Group {
                column: "TextValue",
                number: 1,
                updates: vec![],
            },
            Group {
                column: "BinaryValue",
                number: 2,
                updates: vec![],
            },
        ],
        Recipe::TextOverflow => vec![t(2, text(""), text("雪雪"), &[234, 150, 234, 150])],
        Recipe::SharedOverflow => vec![
            t(
                0,
                alpha(),
                text("Ω雪🙂a"),
                &[169, 3, 234, 150, 61, 216, 66, 222, 97, 0],
            ),
            grow_binary(),
        ],
        Recipe::StaleText => vec![t(
            0,
            text("wrong"),
            text("beta"),
            &[255, 254, b'b', b'e', b't', b'a'],
        )],
        Recipe::StaleBinary => vec![b(0, binary(&[1]), binary(&[]), &[])],
    };
    let refusal = match recipe {
        Recipe::TextOverflow | Recipe::SharedOverflow => Some(Refusal::Capacity),
        Recipe::StaleText | Recipe::StaleBinary => Some(Refusal::Conflict),
        _ => None,
    };
    (groups, refusal)
}

/// Complete independently authored original and optional literal AFTER image.
pub fn source(header: u8, recipe: Recipe) -> Case {
    assert!(matches!(header, 5 | 6));
    let mut before = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    before[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    before[7 * PAGE..8 * PAGE].copy_from_slice(&definition());
    let mut data = vec![0; PAGE];
    data[..2].copy_from_slice(&[1, 1]);
    put32(&mut data, 4, 7);
    put16(&mut data, 12, 3);
    let mut stop = PAGE;
    let mut frames = Vec::new();
    for row in 0..3 {
        let (record, starts) = original_record(row);
        stop -= record.len();
        data[stop..stop + record.len()].copy_from_slice(&record);
        put16(&mut data, 14 + row * 2, stop);
        frames.push(Frame {
            absolute: 8 * PAGE + stop,
            length: record.len(),
            starts,
            data_end: record.len() - FOOTER,
        });
    }
    put16(&mut data, 2, stop - 20);
    before[8 * PAGE..9 * PAGE].copy_from_slice(&data);
    let rows = rows();
    let (groups, refusal) = requests(recipe);
    let mut final_rows = rows.clone();
    let expected = if refusal.is_some() {
        None
    } else {
        let mut after = before.clone();
        for (row, frame) in frames.iter().enumerate() {
            let original = &before[frame.absolute..frame.absolute + frame.length];
            let target = &mut after[frame.absolute..frame.absolute + frame.length];
            let mut selected: [Option<&Update>; 8] = [None; 8];
            for group in &groups {
                for update in &group.updates {
                    if update.row == row {
                        assert_eq!(update.expected, rows[row][group.number]);
                        assert!(selected[group.number].replace(update).is_none());
                    }
                }
            }
            let mut cursor = FIXED_END;
            let mut offsets = [0; 4];
            for slot in 0..3 {
                offsets[slot] = cursor;
                let payload = selected[slot + 1]
                    .filter(|u| u.expected != u.replacement)
                    .map(|u| u.payload.unwrap())
                    .unwrap_or(&original[frame.starts[slot]..frame.starts[slot + 1]]);
                assert!(cursor + payload.len() <= frame.data_end);
                target[cursor..cursor + payload.len()].copy_from_slice(payload);
                cursor += payload.len();
            }
            offsets[3] = cursor;
            for (slot, offset) in offsets.into_iter().enumerate() {
                put16(target, frame.length - 5 - slot * 2, offset);
            }
            for (number, update) in selected.into_iter().enumerate() {
                let Some(update) = update else {
                    continue;
                };
                if update.expected == update.replacement {
                    continue;
                }
                final_rows[row][number] = update.replacement.clone();
                let bit = 1 << number;
                let present = if number == 4 {
                    matches!(update.replacement, Value::Boolean(true))
                } else {
                    !matches!(update.replacement, Value::Null)
                };
                target[frame.length - 1] =
                    (target[frame.length - 1] & !bit) | if present { bit } else { 0 };
                let payload = match &update.replacement {
                    Value::Int32(v) => Some((2, v.to_le_bytes().to_vec())),
                    Value::Int64(v) => Some((6, v.to_le_bytes().to_vec())),
                    Value::Currency(v) => Some((14, v.scaled().to_le_bytes().to_vec())),
                    Value::DateTime(v) => Some((22, v.to_bits().to_le_bytes().to_vec())),
                    _ => None,
                };
                if let Some((at, bytes)) = payload {
                    target[at..at + bytes.len()].copy_from_slice(&bytes);
                }
            }
        }
        Some(after)
    };
    let changed = expected.as_ref().map_or(0, |after| {
        frames
            .iter()
            .filter(|f| {
                before[f.absolute..f.absolute + f.length]
                    != after[f.absolute..f.absolute + f.length]
            })
            .count()
    });
    Case {
        before,
        expected,
        rows,
        final_rows,
        groups,
        frames,
        changed,
        refusal,
    }
}

/// Compare the complete complement and original record-local slack geometry.
pub fn assert_complement(case: &Case) {
    let Some(after) = &case.expected else {
        return;
    };
    assert_eq!(after.len(), case.before.len());
    let mut ranges = case
        .frames
        .iter()
        .map(|frame| frame.absolute..frame.absolute + frame.length)
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| range.start);
    let mut previous = 0;
    for range in ranges {
        assert_eq!(
            &after[previous..range.start],
            &case.before[previous..range.start]
        );
        previous = range.end;
    }
    assert_eq!(&after[previous..], &case.before[previous..]);
    for frame in &case.frames {
        assert_eq!(&after[frame.absolute..frame.absolute + 2], &[8, 0]);
        let end = frame.absolute + frame.length;
        assert_eq!(&after[end - 3..end - 1], &[3, 0]);
        assert_eq!(frame.data_end, frame.length - 11);
    }
}
