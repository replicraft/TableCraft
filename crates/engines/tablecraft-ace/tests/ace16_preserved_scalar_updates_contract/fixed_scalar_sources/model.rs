//! Literal eight-byte preservation expectations from the unchanged rich owner.
use super::rich;
use tablecraft_core::values::{Currency, LegacyDateTime, Value};

const PAGE: usize = 4096;
const DATA_PAGE: usize = 12;
const WIDTH: usize = 8;
const INACTIVE_DATE: [u8; 8] = [255; 8];
const INACTIVE_INTEGER: [u8; 8] = [145, 130, 115, 100, 85, 70, 55, 40];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    LargeNumber,
    Date,
    Time,
    Currency,
}

pub(crate) const TARGETS: [Target; 4] = [
    Target::LargeNumber,
    Target::Date,
    Target::Time,
    Target::Currency,
];

impl Target {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::LargeNumber => "Large_Number",
            Self::Date => "Date_Field",
            Self::Time => "Time_Field",
            Self::Currency => "Currency_Field",
        }
    }

    pub(crate) fn number(self) -> usize {
        match self {
            Self::LargeNumber => 4,
            Self::Date => 5,
            Self::Time => 6,
            Self::Currency => 9,
        }
    }

    pub(crate) fn offset(self) -> usize {
        match self {
            Self::LargeNumber => 8,
            Self::Date => 16,
            Self::Time => 24,
            Self::Currency => 116,
        }
    }

    pub(crate) fn native(self) -> u8 {
        match self {
            Self::LargeNumber => 19,
            Self::Date | Self::Time => 8,
            Self::Currency => 5,
        }
    }

    pub(crate) fn value(self, bytes: Option<[u8; 8]>) -> Value {
        let Some(bytes) = bytes else {
            return Value::Null;
        };
        match self {
            Self::LargeNumber => Value::Int64(i64::from_le_bytes(bytes)),
            Self::Currency => Value::Currency(Currency::from_scaled(i64::from_le_bytes(bytes))),
            Self::Date | Self::Time => {
                Value::DateTime(LegacyDateTime::from_bits(u64::from_le_bytes(bytes)).unwrap())
            }
        }
    }

    fn original(self, row: usize) -> [u8; 8] {
        assert!(row < 3);
        match self {
            Self::LargeNumber => [
                [232, 3, 0, 0, 0, 0, 0, 0],
                [208, 7, 0, 0, 0, 0, 0, 0],
                [184, 11, 0, 0, 0, 0, 0, 0],
            ][row],
            Self::Date => [0; 8],
            Self::Time => [0, 0, 0, 0, 0, 0, 224, 63],
            Self::Currency => [
                [57, 48, 0, 0, 0, 0, 0, 0],
                [114, 96, 0, 0, 0, 0, 0, 0],
                [171, 144, 0, 0, 0, 0, 0, 0],
            ][row],
        }
    }

    fn changed(self) -> [u8; 8] {
        match self {
            Self::LargeNumber => [155, 255, 255, 255, 255, 255, 255, 255],
            Self::Date => [0, 0, 0, 0, 0, 0, 244, 63],
            Self::Time => [0, 0, 0, 0, 0, 0, 208, 191],
            Self::Currency => [239, 216, 255, 255, 255, 255, 255, 255],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Recipe {
    Change,
    NullToValue,
    NullToSameBytes,
    ValueToNull,
    NullNoop,
    ValueNoop,
    MixedRows,
    Minimum,
    Maximum,
    SignedZero,
    TemporalAlias,
    NegativeZeroNoop,
    RequiredRefusal,
}

pub(crate) fn cases() -> Vec<(Target, Recipe)> {
    let mut cases = Vec::new();
    for target in TARGETS {
        for recipe in [
            Recipe::Change,
            Recipe::NullToValue,
            Recipe::NullToSameBytes,
            Recipe::ValueToNull,
            Recipe::NullNoop,
            Recipe::ValueNoop,
            Recipe::MixedRows,
            Recipe::RequiredRefusal,
        ] {
            cases.push((target, recipe));
        }
    }
    for target in [Target::LargeNumber, Target::Currency] {
        cases.push((target, Recipe::Minimum));
        cases.push((target, Recipe::Maximum));
    }
    cases.push((Target::Date, Recipe::SignedZero));
    cases.push((Target::Date, Recipe::NegativeZeroNoop));
    cases.push((Target::Date, Recipe::TemporalAlias));
    cases
}

pub(crate) struct Case {
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub target: Target,
    pub initial: [Option<[u8; 8]>; 3],
    pub replacement: [Option<[u8; 8]>; 3],
    pub requested_rows: usize,
    pub changed: usize,
    pub required_patch: Option<usize>,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn integer(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

pub(crate) fn location(bytes: &[u8], target: Target, row: usize) -> (usize, usize) {
    assert!(row < 3);
    let page = DATA_PAGE * PAGE;
    assert_eq!(word(bytes, page + 12), 3);
    let start = word(bytes, page + 14 + row * 2);
    let end = if row == 0 {
        PAGE
    } else {
        word(bytes, page + 12 + row * 2)
    };
    assert_eq!(start & 0xe000, 0);
    assert!(20 <= start && start < end && end <= PAGE);
    assert_eq!(word(bytes, page + start), 16);
    let payload = page + start + 2 + target.offset();
    let mask = page + end - 2 + target.number() / 8;
    assert!(payload + WIDTH <= page + start + 130);
    assert!(payload + WIDTH < page + end - 2);
    (payload, mask)
}

fn unicode(bytes: &[u8]) -> String {
    let (units, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    String::from_utf16(
        &units
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn required(bytes: &mut [u8], target: Target) -> usize {
    let locations = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(at, value)| (value == b"MR2\0").then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(locations.len(), 1);
    let start = locations[0];
    assert!(start >= 12);
    let envelope = integer(bytes, start - 12);
    assert_eq!(envelope & 0xc000_0000, 0x8000_0000);
    let end = start + (envelope & 0x3fff_ffff);
    assert!(end <= bytes.len());
    assert_eq!(word(bytes, start + 8), 0x80);
    let names_end = start + 4 + integer(bytes, start + 4);
    let mut at = start + 10;
    let mut names = Vec::new();
    while at < names_end {
        let width = word(bytes, at);
        at += 2;
        assert!(at + width <= names_end);
        names.push(unicode(&bytes[at..at + width]));
        at += width;
    }
    assert_eq!(at, names_end);
    let required_name = names.iter().position(|name| name == "Required").unwrap();
    let mut patch = None;
    while at < end {
        let next = at + integer(bytes, at);
        assert!(next > at + 6 && next <= end);
        let name_width = word(bytes, at + 10);
        let name_end = at + 12 + name_width;
        assert_eq!(integer(bytes, at + 6), 6 + name_width);
        assert!(name_end <= next);
        if word(bytes, at + 4) == 1 && unicode(&bytes[at + 12..name_end]) == target.name() {
            let mut entry = name_end;
            while entry < next {
                let width = word(bytes, entry);
                assert!(width >= 8 && entry + width <= next);
                if word(bytes, entry + 4) == required_name {
                    assert!(patch.replace(entry + 8).is_none());
                    assert_eq!(width, 9);
                    assert_eq!(&bytes[entry + 2..entry + 4], &[1, 1]);
                    assert_eq!(word(bytes, entry + 6), 1);
                    assert_eq!(bytes[entry + 8], 0);
                }
                entry += width;
            }
            assert_eq!(entry, next);
        }
        at = next;
    }
    assert_eq!(at, end);
    let patch = patch.unwrap();
    bytes[patch] = 1;
    patch
}

pub(crate) fn source(code: u8, target: Target, recipe: Recipe) -> Case {
    assert!(matches!(code, 5 | 6));
    assert!(cases().contains(&(target, recipe)));
    let mut before = rich::source().before;
    assert_eq!(before[20], 6);
    before[20] = code;
    let definition = 10 * PAGE;
    assert_eq!(word(&before, definition + 45), 16);
    let physical = integer(&before, definition + 51);
    assert_eq!(physical, 3);
    let descriptor = definition + 63 + physical * 12 + target.number() * 25;
    assert_eq!(before[descriptor], target.native());
    assert_eq!(word(&before, descriptor + 5), target.number());
    assert_eq!(&before[descriptor + 15..descriptor + 17], &[3, 0]);
    assert_eq!(word(&before, descriptor + 21), target.offset());
    assert_eq!(word(&before, descriptor + 23), 8);
    let mut initial = std::array::from_fn(|row| Some(target.original(row)));
    let mut replacement = initial;
    match recipe {
        Recipe::Change => replacement[0] = Some(target.changed()),
        Recipe::NullToValue => {
            initial[0] = None;
            replacement[0] = Some(target.changed());
        }
        Recipe::NullToSameBytes => {
            initial[0] = None;
            replacement[0] = Some(target.original(0));
        }
        Recipe::ValueToNull | Recipe::RequiredRefusal => replacement[0] = None,
        Recipe::NullNoop => {
            initial[0] = None;
            replacement[0] = None;
        }
        Recipe::ValueNoop => {}
        Recipe::MixedRows => {
            initial[0] = None;
            replacement[0] = Some(target.changed());
            replacement[1] = None;
        }
        Recipe::Minimum => replacement[0] = Some([0, 0, 0, 0, 0, 0, 0, 128]),
        Recipe::Maximum => replacement[0] = Some([255, 255, 255, 255, 255, 255, 255, 127]),
        Recipe::SignedZero => replacement[0] = Some([0, 0, 0, 0, 0, 0, 0, 128]),
        Recipe::TemporalAlias => {
            initial[0] = Some([0, 0, 0, 0, 0, 0, 208, 63]);
            replacement[0] = Some([0, 0, 0, 0, 0, 0, 208, 191]);
        }
        Recipe::NegativeZeroNoop => {
            initial[0] = Some([0, 0, 0, 0, 0, 0, 0, 128]);
            replacement[0] = initial[0];
        }
    }
    let bit = 1 << (target.number() % 8);
    for (row, initial_value) in initial.iter().copied().enumerate() {
        let (payload, mask) = location(&before, target, row);
        assert_ne!(before[mask] & bit, 0);
        assert_eq!(&before[payload..payload + WIDTH], &target.original(row));
        if let Some(value) = initial_value {
            before[payload..payload + WIDTH].copy_from_slice(&value);
        } else {
            before[mask] &= !bit;
            let inactive = if recipe == Recipe::NullToSameBytes {
                target.original(row)
            } else if matches!(target, Target::Date | Target::Time) {
                INACTIVE_DATE
            } else {
                INACTIVE_INTEGER
            };
            before[payload..payload + WIDTH].copy_from_slice(&inactive);
        }
    }
    let required_patch = if recipe == Recipe::RequiredRefusal {
        Some(required(&mut before, target))
    } else {
        None
    };
    let requested_rows = if recipe == Recipe::MixedRows { 3 } else { 1 };
    let changed = initial
        .iter()
        .zip(replacement)
        .take(requested_rows)
        .filter(|(old, new)| **old != *new)
        .count();
    let expected = if required_patch.is_some() {
        None
    } else {
        let mut after = before.clone();
        for (row, replacement_value) in replacement.iter().copied().enumerate().take(requested_rows)
        {
            let (payload, mask) = location(&before, target, row);
            if let Some(value) = replacement_value {
                after[mask] |= bit;
                after[payload..payload + WIDTH].copy_from_slice(&value);
            } else {
                after[mask] &= !bit;
            }
        }
        Some(after)
    };
    Case {
        before,
        expected,
        target,
        initial,
        replacement,
        requested_rows,
        changed,
        required_patch,
    }
}

pub(crate) fn assert_complement(case: &Case) {
    let Some(after) = &case.expected else {
        assert!(case.required_patch.is_some());
        return;
    };
    assert_eq!(case.before.len(), after.len());
    let mut allowed = Vec::new();
    let bit = 1 << (case.target.number() % 8);
    for row in 0..3 {
        let (_, mask) = location(&case.before, case.target, row);
        assert_eq!(case.before[mask] & bit != 0, case.initial[row].is_some());
        assert_eq!(after[mask] & bit != 0, case.replacement[row].is_some());
    }
    for row in 0..case.requested_rows {
        let (payload, mask) = location(&case.before, case.target, row);
        allowed.extend(payload..payload + WIDTH);
        allowed.push(mask);
        assert_eq!((case.before[mask] ^ after[mask]) & !bit, 0);
        if case.replacement[row].is_none() || case.initial[row] == case.replacement[row] {
            assert_eq!(
                &case.before[payload..payload + WIDTH],
                &after[payload..payload + WIDTH]
            );
        }
    }
    for (at, (old, new)) in case.before.iter().zip(after).enumerate() {
        assert!(
            old == new || allowed.contains(&at),
            "outside selected fixed payload/mask at {at}"
        );
    }
    assert_eq!(
        case.changed,
        case.initial
            .iter()
            .zip(case.replacement)
            .take(case.requested_rows)
            .filter(|(old, new)| **old != *new)
            .count()
    );
    if case.changed == 0 {
        assert_eq!(&case.before, after);
    }
}
