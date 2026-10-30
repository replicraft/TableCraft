//! Fixed civil inputs and literal DATE bits; no runtime clock or date encoder.
use super::super::{
    multiple_indexes::model as frames, ordinary_rows::fixture,
    passive_defaults::model as properties,
};
use crate::test_support::base;
use properties::{Property, field};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};
use tablecraft_testkit::access as raw;

pub(super) const PAGE: usize = 4096;

#[derive(Clone, Copy)]
pub(super) enum Reading {
    LeapNoon,
    BeforeEpochSix,
    EpochSix,
    Minimum,
    Maximum,
    MidnightFirst,
}

impl Reading {
    pub(super) fn input(self) -> ExtendedDateTime {
        let (year, month, day, hour, minute, second, fraction) = match self {
            Self::LeapNoon => (2000, 2, 29, 12, 0, 0, 7_654_321),
            Self::BeforeEpochSix => (1899, 12, 29, 6, 0, 0, 0),
            Self::EpochSix => (1899, 12, 30, 6, 0, 0, 0),
            Self::Minimum => (100, 1, 1, 0, 0, 0, 0),
            Self::Maximum => (9999, 12, 31, 23, 59, 59, 9_999_999),
            Self::MidnightFirst => (2000, 2, 29, 23, 59, 59, 0),
        };
        ExtendedDateTime::new(year, month, day, hour, minute, second, fraction).unwrap()
    }

    pub(super) fn bits(self) -> (u64, u64) {
        // Independent integer Gregorian-day arithmetic and fixed IEEE literals.
        // Fractions in input() do not enter DATE/Now's whole-second projection.
        match self {
            Self::LeapNoon => (0x40e1dd2000000000, 0x40e1dd3000000000),
            Self::BeforeEpochSix => (0xbff0000000000000, 0xbff4000000000000),
            Self::EpochSix => (0x0000000000000000, 0x3fd0000000000000),
            Self::Minimum => (0xc124103400000000, 0xc124103400000000),
            Self::Maximum => (0x4146924080000000, 0x41469240ffff9ee9),
            Self::MidnightFirst => (0x40e1dd2000000000, 0x40e1dd3fffe7ba37),
        }
    }
}

#[derive(Clone)]
pub(super) struct Row {
    pub dated: Option<u64>,
    pub id: i32,
    pub stamped: Option<u64>,
}

impl Row {
    pub(super) fn values(&self) -> Vec<Value> {
        vec![
            self.dated.map_or(Value::Null, |bits| {
                Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
            }),
            Value::Int32(self.id),
            self.stamped.map_or(Value::Null, |bits| {
                Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
            }),
        ]
    }
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<Row>,
    pub generated: bool,
}

fn record(row: &Row, old: bool) -> Vec<u8> {
    let absent = if old { [0xa5; 8] } else { [0; 8] };
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.dated.map_or(absent, u64::to_le_bytes));
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.stamped.map_or(absent, u64::to_le_bytes));
    bytes.push(u8::from(row.dated.is_some()) | 2 | (u8::from(row.stamped.is_some()) << 2));
    assert_eq!(bytes.len(), 23);
    bytes
}

fn keys(rows: &[Row]) -> Vec<Vec<u8>> {
    let mut result = rows
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut wire = vec![127];
            wire.extend(((row.id as u32) ^ 0x80000000).to_be_bytes());
            wire.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
            wire
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}

pub(super) fn source(code: u8, generated: bool, dated: &str, stamped: &str) -> Case {
    let original = fixture::source(code, false, false, fixture::Index::Alias, false);
    let mut before = original.before.clone();
    let old = &original.before[10 * PAGE..11 * PAGE];
    let fields = [("Dated", 8, 0, 8), ("ID", 4, 8, 4), ("Stamped", 8, 12, 8)];
    let mut definition = base::definition(&fields, 2, 11 << 8);
    let mut end = 63 + fields.len() * 25;
    for (name, _, _, _) in fields {
        end += 2 + name.encode_utf16().count() * 2;
    }
    definition.copy_within(63..end, 75);
    definition[63..75].fill(0);
    raw::put32(&mut definition, 67, 17);
    raw::put32(&mut definition, 47, 2);
    raw::put32(&mut definition, 51, 1);
    raw::put32(&mut definition, 59, (11 << 8) | 1);
    if generated {
        definition[75 + 25 + 15] = 7;
        raw::put32(&mut definition, 20, 260);
    }
    let physical = end + 12;
    definition[physical..physical + 52]
        .copy_from_slice(&old[original.physical..original.physical + 52]);
    raw::put16(&mut definition, physical + 4, 1);
    let logical = physical + 52;
    definition[logical..logical + 56]
        .copy_from_slice(&old[original.logical..original.logical + 56]);
    let mut destination = logical + 56;
    let mut origin = original.logical + 56;
    for _ in 0..2 {
        let length = 2 + usize::from(u16::from_le_bytes(
            old[origin..origin + 2].try_into().unwrap(),
        ));
        definition[destination..destination + length]
            .copy_from_slice(&old[origin..origin + length]);
        destination += length;
        origin += length;
    }
    raw::put16(&mut definition, destination, 0xffff);
    before[10 * PAGE..11 * PAGE].copy_from_slice(&definition);

    let rows = vec![
        Row {
            dated: Some(0x8000000000000000),
            id: 1,
            stamped: None,
        },
        Row {
            dated: None,
            id: 2,
            stamped: Some(0xbff4000000000000),
        },
    ];
    let records = rows
        .iter()
        .map(|row| (0, record(row, true)))
        .collect::<Vec<_>>();
    let mut data = raw::data_page(10, &records);
    raw::put16(&mut data, 2, 4032);
    before[12 * PAGE..13 * PAGE].copy_from_slice(&data);
    let leaf = frames::leaf(Some(&before[13 * PAGE..14 * PAGE]), &keys(&rows), 0, 0, 0);
    before[13 * PAGE..14 * PAGE].copy_from_slice(&leaf);
    properties::properties(
        &mut before,
        &[
            field("Dated", vec![Property::text("DefaultValue", dated)]),
            field("ID", vec![Property::boolean("Required", true)]),
            field("Stamped", vec![Property::text("DefaultValue", stamped)]),
        ],
    );
    Case {
        before,
        rows,
        generated,
    }
}

pub(super) fn maximum_counter_source(code: u8) -> Case {
    let mut case = source(code, true, "Date()", "Now()");
    case.before[10 * PAGE + 20..10 * PAGE + 24].copy_from_slice(&i32::MAX.to_le_bytes());
    case
}

pub(super) fn added(case: &Case, reading: Reading) -> [Row; 2] {
    let (dated, stamped) = reading.bits();
    let first = if case.generated { 261 } else { 10 };
    [
        Row {
            dated: Some(dated),
            id: first,
            stamped: Some(stamped),
        },
        Row {
            dated: Some(dated),
            id: first + 1,
            stamped: Some(stamped),
        },
    ]
}

pub(super) fn wrong_target_source(code: u8) -> Case {
    let mut case = source(code, false, "Date()", "Now()");
    properties::properties(
        &mut case.before,
        &[
            field("Dated", vec![Property::text("DefaultValue", "Date()")]),
            field(
                "ID",
                vec![
                    Property::boolean("Required", true),
                    Property::text("DefaultValue", "Now()"),
                ],
            ),
            field("Stamped", vec![Property::text("DefaultValue", "Now()")]),
        ],
    );
    case
}

pub(super) fn expected(case: &Case, added: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    for row in added {
        let base = 12 * PAGE;
        let count = usize::from(u16::from_le_bytes(
            bytes[base + 12..base + 14].try_into().unwrap(),
        ));
        let stop = usize::from(u16::from_le_bytes(
            bytes[base + 12 + count * 2..base + 14 + count * 2]
                .try_into()
                .unwrap(),
        ));
        let wire = record(row, false);
        let start = stop - wire.len();
        bytes[base + start..base + stop].copy_from_slice(&wire);
        raw::put16(&mut bytes, base + 14 + count * 2, start as u16);
        raw::put16(&mut bytes, base + 12, (count + 1) as u16);
        raw::put16(&mut bytes, base + 2, (start - 14 - (count + 1) * 2) as u16);
        rows.push(row.clone());
    }
    let leaf = frames::leaf(
        Some(&case.before[13 * PAGE..14 * PAGE]),
        &keys(&rows),
        0,
        0,
        0,
    );
    bytes[13 * PAGE..14 * PAGE].copy_from_slice(&leaf);
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    raw::put32(&mut bytes, 10 * PAGE + 67, 17 + added.len() as u32);
    if case.generated
        && let Some(last) = added.last()
    {
        raw::put32(&mut bytes, 10 * PAGE + 20, last.id as u32);
    }
    bytes
}
