//! Independent fixed payload/mask overlays on the actual rich source owner.
use self::{
    Cell::{Boolean as B, Currency as C, Date as D, Int32 as I, Int64 as L, Null as N},
    Column::{Currency as CurrencyColumn, Date, IndexedNumber, Large, LargeInt32, No, Time, Yes},
};
use super::{fixed_scalar_model, int32_model, rich};
use std::collections::{BTreeMap, BTreeSet};
use tablecraft_core::values::{Currency, LegacyDateTime, Value};

const PAGE: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const DATA_PAGE: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Column {
    Large,
    LargeInt32,
    Date,
    Time,
    Currency,
    Yes,
    No,
    IndexedNumber,
}

pub(crate) const COLUMNS: [Column; 7] = [
    Column::Large,
    Column::Date,
    Column::Time,
    Column::Currency,
    Column::Yes,
    Column::No,
    Column::IndexedNumber,
];

impl Column {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Large | Self::LargeInt32 => "Large_Number",
            Self::Date => "Date_Field",
            Self::Time => "Time_Field",
            Self::Currency => "Currency_Field",
            Self::Yes => "Yes_Field",
            Self::No => "No_Field",
            Self::IndexedNumber => "Number",
        }
    }

    pub(crate) fn number(self) -> usize {
        match self {
            Self::Large | Self::LargeInt32 => 4,
            Self::Date => 5,
            Self::Time => 6,
            Self::Currency => 9,
            Self::Yes => 10,
            Self::No => 11,
            Self::IndexedNumber => 3,
        }
    }

    pub(crate) fn native(self) -> u8 {
        match self {
            Self::Large => 19,
            Self::Date | Self::Time => 8,
            Self::Currency => 5,
            Self::Yes | Self::No => 1,
            Self::IndexedNumber | Self::LargeInt32 => 4,
        }
    }

    pub(crate) fn offset(self) -> usize {
        match self {
            Self::Large | Self::LargeInt32 => 8,
            Self::Date => 16,
            Self::Time => 24,
            Self::Currency => 116,
            Self::Yes | Self::No => 0,
            Self::IndexedNumber => 4,
        }
    }

    pub(crate) fn width(self) -> usize {
        match self {
            Self::Yes | Self::No => 0,
            Self::IndexedNumber | Self::LargeInt32 => 4,
            _ => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Cell {
    Null,
    Boolean(bool),
    Int32(i32),
    Int64(i64),
    Currency(i64),
    Date(u64),
}

impl Cell {
    pub(crate) fn value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Boolean(value) => Value::Boolean(value),
            Self::Int32(value) => Value::Int32(value),
            Self::Int64(value) => Value::Int64(value),
            Self::Currency(value) => Value::Currency(Currency::from_scaled(value)),
            Self::Date(bits) => Value::DateTime(LegacyDateTime::from_bits(bits).unwrap()),
        }
    }

    fn bytes(self) -> Vec<u8> {
        match self {
            Self::Int32(value) => value.to_le_bytes().to_vec(),
            Self::Int64(value) | Self::Currency(value) => value.to_le_bytes().to_vec(),
            Self::Date(bits) => bits.to_le_bytes().to_vec(),
            Self::Null | Self::Boolean(_) => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Assignment {
    pub row: usize,
    pub expected: Cell,
    pub replacement: Cell,
}

#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub table: &'static str,
    pub name: &'static str,
    pub column: Column,
    pub updates: Vec<Assignment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Recipe {
    BooleanAndLarge,
    ThreePayloads,
    SharedLowMask,
    SharedHighMask,
    OverlapRows,
    MixedNoops,
    ExactNoops,
    InactiveNullNoops,
    AllNullPromoteSamePayload,
    AllNullStay,
    MixedNullTransitions,
    EmptyNumericAndBoolean,
    EntirelyEmpty,
    EmptyBooleanAndNumeric,
    DateAliases,
    RepeatedRowAcrossColumns,
    Int32Boolean,
    Int32CurrencyDate,
    Int32NullPadding,
    LateStale,
    LateRequired,
    LateIndexed,
    LateWrongCarrier,
    DuplicateColumn,
    DuplicateRow,
    InvalidOrdinal,
    EmptyIneligible,
}

pub(crate) const RECIPES: [Recipe; 27] = [
    Recipe::BooleanAndLarge,
    Recipe::ThreePayloads,
    Recipe::SharedLowMask,
    Recipe::SharedHighMask,
    Recipe::OverlapRows,
    Recipe::MixedNoops,
    Recipe::ExactNoops,
    Recipe::InactiveNullNoops,
    Recipe::AllNullPromoteSamePayload,
    Recipe::AllNullStay,
    Recipe::MixedNullTransitions,
    Recipe::EmptyNumericAndBoolean,
    Recipe::EntirelyEmpty,
    Recipe::EmptyBooleanAndNumeric,
    Recipe::DateAliases,
    Recipe::RepeatedRowAcrossColumns,
    Recipe::Int32Boolean,
    Recipe::Int32CurrencyDate,
    Recipe::Int32NullPadding,
    Recipe::LateStale,
    Recipe::LateRequired,
    Recipe::LateIndexed,
    Recipe::LateWrongCarrier,
    Recipe::DuplicateColumn,
    Recipe::DuplicateRow,
    Recipe::InvalidOrdinal,
    Recipe::EmptyIneligible,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    Conflict,
    Required,
    IndexedColumn,
    Column,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Flavor {
    Rich,
    Int32,
}

pub(crate) fn columns(flavor: Flavor) -> [Column; 7] {
    let mut columns = COLUMNS;
    if flavor == Flavor::Int32 {
        columns[0] = Column::LargeInt32;
    }
    columns
}

pub(crate) struct Case {
    pub flavor: Flavor,
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub groups: Vec<Group>,
    pub changed: usize,
    pub refusal: Option<Refusal>,
    pub required_patch: Option<usize>,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub(crate) fn location(bytes: &[u8], column: Column, row: usize) -> (usize, usize) {
    assert!(row < 3);
    let page = DATA_PAGE * PAGE;
    assert_eq!(word(bytes, page + 12), 3);
    let first = word(bytes, page + 14 + row * 2);
    let last = if row == 0 {
        PAGE
    } else {
        word(bytes, page + 12 + row * 2)
    };
    assert_eq!(first & 0xe000, 0);
    assert!(20 <= first && first < last && last <= PAGE);
    assert_eq!(word(bytes, page + first), 16);
    let at = page + first + 2 + column.offset();
    let mask = page + last - 2 + column.number() / 8;
    if column.width() != 0 {
        assert!(at + column.width() <= page + first + 130);
    }
    (at, mask)
}

pub(crate) fn read(bytes: &[u8], column: Column, row: usize) -> Cell {
    let (at, mask) = location(bytes, column, row);
    let bit = 1 << (column.number() % 8);
    if matches!(column, Column::Yes | Column::No) {
        return Cell::Boolean(bytes[mask] & bit != 0);
    }
    if bytes[mask] & bit == 0 {
        return Cell::Null;
    }
    match column {
        Column::Large => Cell::Int64(i64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())),
        Column::Currency => {
            Cell::Currency(i64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()))
        }
        Column::Date | Column::Time => {
            Cell::Date(u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()))
        }
        Column::IndexedNumber | Column::LargeInt32 => {
            Cell::Int32(i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()))
        }
        Column::Yes | Column::No => unreachable!(),
    }
}

fn apply(bytes: &mut [u8], column: Column, row: usize, value: Cell) {
    assert!(matches!(
        (column, value),
        (Column::Large, Cell::Int64(_))
            | (Column::LargeInt32 | Column::IndexedNumber, Cell::Int32(_))
            | (Column::Currency, Cell::Currency(_))
            | (Column::Date | Column::Time, Cell::Date(_))
            | (Column::Yes | Column::No, Cell::Boolean(_))
            | (
                Column::Large
                    | Column::LargeInt32
                    | Column::IndexedNumber
                    | Column::Currency
                    | Column::Date
                    | Column::Time,
                Cell::Null
            )
    ));
    let (at, mask) = location(bytes, column, row);
    let bit = 1 << (column.number() % 8);
    match value {
        Cell::Boolean(value) => {
            assert!(matches!(column, Column::Yes | Column::No));
            if value {
                bytes[mask] |= bit;
            } else {
                bytes[mask] &= !bit;
            }
        }
        Cell::Null => {
            assert!(!matches!(column, Column::Yes | Column::No));
            bytes[mask] &= !bit;
        }
        value => {
            let value = value.bytes();
            assert_eq!(value.len(), column.width());
            bytes[at..at + value.len()].copy_from_slice(&value);
            bytes[mask] |= bit;
        }
    }
}

fn group(column: Column, updates: &[(usize, Cell, Cell)]) -> Group {
    Group {
        table: "Items",
        name: column.name(),
        column,
        updates: updates
            .iter()
            .map(|&(row, expected, replacement)| Assignment {
                row,
                expected,
                replacement,
            })
            .collect(),
    }
}

fn absent(bytes: &mut [u8], column: Column, row: usize, invalid_date: bool) {
    apply(bytes, column, row, Cell::Null);
    if invalid_date {
        assert!(matches!(column, Column::Date | Column::Time));
        let (at, _) = location(bytes, column, row);
        bytes[at..at + 8].fill(255);
    }
}

pub(crate) fn reconstruct(before: &[u8], groups: &[Group]) -> (Vec<u8>, usize) {
    let mut after = before.to_vec();
    let mut columns = BTreeSet::new();
    let mut changed = BTreeSet::new();
    for group in groups {
        assert_eq!(group.table, "Items");
        assert_eq!(
            group.name.to_ascii_lowercase(),
            group.column.name().to_ascii_lowercase()
        );
        assert!(columns.insert(group.column.number()));
        let mut rows = BTreeSet::new();
        for assignment in &group.updates {
            assert!(rows.insert(assignment.row));
            assert_eq!(
                read(before, group.column, assignment.row),
                assignment.expected
            );
            if assignment.expected != assignment.replacement {
                apply(
                    &mut after,
                    group.column,
                    assignment.row,
                    assignment.replacement,
                );
                changed.insert(assignment.row);
            }
        }
    }
    (after, changed.len())
}

pub(crate) fn source(code: u8, recipe: Recipe) -> Case {
    assert!(matches!(code, 5 | 6));
    let flavor = if matches!(
        recipe,
        Recipe::Int32Boolean | Recipe::Int32CurrencyDate | Recipe::Int32NullPadding
    ) {
        Flavor::Int32
    } else {
        Flavor::Rich
    };
    let mut before = if flavor == Flavor::Int32 {
        int32_model::source(code, 101).before
    } else {
        rich::source().before
    };
    assert_eq!(before[20], if flavor == Flavor::Int32 { code } else { 6 });
    before[20] = code;
    let definition = DEFINITION_PAGE * PAGE;
    let physical = u32::from_le_bytes(before[definition + 51..definition + 55].try_into().unwrap());
    assert_eq!(physical, 3);
    for column in columns(flavor) {
        let at = definition + 63 + physical as usize * 12 + column.number() * 25;
        assert_eq!(before[at], column.native());
        assert_eq!(word(&before, at + 5), column.number());
        assert_eq!(word(&before, at + 21), column.offset());
        assert_eq!(word(&before, at + 23), column.width());
        assert_eq!(&before[at + 15..at + 17], &[3, 0]);
    }
    let mut refusal = None;
    let mut required_patch = None;
    let groups = match recipe {
        Recipe::BooleanAndLarge => vec![
            group(Yes, &[(0, B(false), B(true))]),
            group(Large, &[(0, L(1000), L(-101))]),
        ],
        Recipe::ThreePayloads => vec![
            group(Large, &[(0, L(1000), L(-101))]),
            group(CurrencyColumn, &[(0, C(12345), C(-10001))]),
            group(Date, &[(0, D(0), D(0x8000_0000_0000_0000))]),
        ],
        Recipe::SharedLowMask => vec![
            group(Date, &[(0, D(0), N)]),
            group(Time, &[(0, D(0x3fe0_0000_0000_0000), N)]),
        ],
        Recipe::SharedHighMask => vec![
            group(CurrencyColumn, &[(0, C(12345), N)]),
            group(Yes, &[(0, B(false), B(true))]),
            group(No, &[(0, B(false), B(true))]),
        ],
        Recipe::OverlapRows => vec![
            group(Large, &[(0, L(1000), L(-101)), (2, L(3000), N)]),
            group(
                CurrencyColumn,
                &[(1, C(24690), C(-10001)), (2, C(37035), C(37035))],
            ),
            group(Yes, &[(0, B(false), B(true)), (1, B(false), B(true))]),
        ],
        Recipe::MixedNoops => vec![
            group(Large, &[(0, L(1000), L(1000)), (1, L(2000), L(-101))]),
            group(Date, &[(0, D(0), D(0))]),
            group(Yes, &[(1, B(false), B(true))]),
        ],
        Recipe::ExactNoops => vec![
            group(Large, &[(0, L(1000), L(1000))]),
            group(CurrencyColumn, &[(0, C(12345), C(12345))]),
            group(Date, &[(0, D(0), D(0))]),
            group(Yes, &[(0, B(false), B(false))]),
        ],
        Recipe::InactiveNullNoops => {
            absent(&mut before, Large, 0, false);
            absent(&mut before, CurrencyColumn, 0, false);
            absent(&mut before, Date, 0, true);
            vec![
                group(Large, &[(0, N, N)]),
                group(CurrencyColumn, &[(0, N, N)]),
                group(Date, &[(0, N, N)]),
                group(Yes, &[(0, B(false), B(false))]),
            ]
        }
        Recipe::AllNullPromoteSamePayload | Recipe::AllNullStay => {
            for column in [Large, CurrencyColumn, Date, Time] {
                absent(&mut before, column, 0, false);
            }
            let values = if recipe == Recipe::AllNullStay {
                [N; 4]
            } else {
                [L(1000), C(12345), D(0), D(0x3fe0_0000_0000_0000)]
            };
            vec![
                group(Large, &[(0, N, values[0])]),
                group(CurrencyColumn, &[(0, N, values[1])]),
                group(Date, &[(0, N, values[2])]),
                group(Time, &[(0, N, values[3])]),
                group(Yes, &[(0, B(false), B(false))]),
            ]
        }
        Recipe::MixedNullTransitions => {
            absent(&mut before, Date, 0, true);
            vec![
                group(Date, &[(0, N, D(0x3fd0_0000_0000_0000))]),
                group(Time, &[(0, D(0x3fe0_0000_0000_0000), N)]),
                group(CurrencyColumn, &[(0, C(12345), N)]),
                group(Yes, &[(0, B(false), B(true))]),
            ]
        }
        Recipe::EmptyNumericAndBoolean => vec![
            group(Large, &[]),
            group(Date, &[]),
            group(Time, &[]),
            group(CurrencyColumn, &[]),
            group(Yes, &[(0, B(false), B(true))]),
        ],
        Recipe::EntirelyEmpty => vec![
            group(Large, &[]),
            group(Date, &[]),
            group(CurrencyColumn, &[]),
            group(Yes, &[]),
            group(No, &[]),
        ],
        Recipe::EmptyBooleanAndNumeric => vec![
            group(Yes, &[]),
            group(Large, &[(0, L(1000), L(-101))]),
            group(No, &[]),
        ],
        Recipe::DateAliases => {
            apply(&mut before, Time, 0, D(0x3fd0_0000_0000_0000));
            vec![
                group(Date, &[(0, D(0), D(0x8000_0000_0000_0000))]),
                group(
                    Time,
                    &[(0, D(0x3fd0_0000_0000_0000), D(0xbfd0_0000_0000_0000))],
                ),
            ]
        }
        Recipe::RepeatedRowAcrossColumns => vec![
            group(Large, &[(0, L(1000), L(-101))]),
            group(CurrencyColumn, &[(0, C(12345), C(-10001))]),
            group(
                Time,
                &[(0, D(0x3fe0_0000_0000_0000), D(0xbfd0_0000_0000_0000))],
            ),
            group(Yes, &[(0, B(false), B(true))]),
        ],
        Recipe::Int32Boolean => vec![
            group(LargeInt32, &[(0, I(1000), I(101))]),
            group(Yes, &[(0, B(false), B(true))]),
        ],
        Recipe::Int32CurrencyDate => vec![
            group(LargeInt32, &[(0, I(1000), I(-101))]),
            group(CurrencyColumn, &[(0, C(12345), C(-10001))]),
            group(Date, &[(0, D(0), D(0x8000_0000_0000_0000))]),
            group(Yes, &[(0, B(false), B(true))]),
        ],
        Recipe::Int32NullPadding => {
            absent(&mut before, LargeInt32, 0, false);
            let (first, _) = location(&before, LargeInt32, 0);
            before[first..first + 4].copy_from_slice(&[0x91, 0x82, 0x73, 0x64]);
            before[first + 4..first + 8].copy_from_slice(&[0xa1, 0xb2, 0xc3, 0xd4]);
            let (second, _) = location(&before, LargeInt32, 1);
            before[second + 4..second + 8].copy_from_slice(&[0xe5, 0xf6, 0x97, 0x88]);
            vec![
                group(LargeInt32, &[(0, N, I(101)), (1, I(2000), N)]),
                group(CurrencyColumn, &[(1, C(24690), C(-10001))]),
                group(Yes, &[(1, B(false), B(true))]),
            ]
        }
        Recipe::LateStale => {
            refusal = Some(Refusal::Conflict);
            vec![
                group(Yes, &[(0, B(false), B(true))]),
                group(Large, &[(0, L(999), L(-101))]),
            ]
        }
        Recipe::LateRequired => {
            let case = fixed_scalar_model::source(
                code,
                fixed_scalar_model::Target::Currency,
                fixed_scalar_model::Recipe::RequiredRefusal,
            );
            before = case.before;
            required_patch = case.required_patch;
            refusal = Some(Refusal::Required);
            vec![
                group(Yes, &[(0, B(false), B(true))]),
                group(CurrencyColumn, &[(0, C(12345), N)]),
            ]
        }
        Recipe::LateIndexed | Recipe::EmptyIneligible => {
            refusal = Some(Refusal::IndexedColumn);
            let indexed = if recipe == Recipe::EmptyIneligible {
                group(IndexedNumber, &[])
            } else {
                group(IndexedNumber, &[(0, I(1), I(12))])
            };
            vec![group(Large, &[(0, L(1000), L(-101))]), indexed]
        }
        Recipe::LateWrongCarrier => {
            refusal = Some(Refusal::Column);
            vec![
                group(Yes, &[(0, B(false), B(true))]),
                group(CurrencyColumn, &[(0, L(12345), L(-101))]),
            ]
        }
        Recipe::DuplicateColumn => {
            refusal = Some(Refusal::Conflict);
            let mut duplicate = group(Large, &[(1, L(2000), L(-101))]);
            duplicate.name = "large_number";
            vec![group(Large, &[(0, L(1000), L(-101))]), duplicate]
        }
        Recipe::DuplicateRow => {
            refusal = Some(Refusal::Conflict);
            vec![
                group(Yes, &[(0, B(false), B(true))]),
                group(
                    CurrencyColumn,
                    &[(0, C(12345), C(-10001)), (0, C(12345), C(-10001))],
                ),
            ]
        }
        Recipe::InvalidOrdinal => {
            refusal = Some(Refusal::Conflict);
            vec![
                group(Yes, &[(0, B(false), B(true))]),
                group(Large, &[(3, N, N)]),
            ]
        }
    };
    let (expected, changed) = if refusal.is_none() {
        let (expected, changed) = reconstruct(&before, &groups);
        (Some(expected), changed)
    } else {
        (None, 0)
    };
    let literal_count = match recipe {
        Recipe::OverlapRows => 3,
        Recipe::Int32NullPadding => 2,
        Recipe::ExactNoops
        | Recipe::InactiveNullNoops
        | Recipe::AllNullStay
        | Recipe::EntirelyEmpty
        | Recipe::LateStale
        | Recipe::LateRequired
        | Recipe::LateIndexed
        | Recipe::LateWrongCarrier
        | Recipe::DuplicateColumn
        | Recipe::DuplicateRow
        | Recipe::InvalidOrdinal
        | Recipe::EmptyIneligible => 0,
        _ => 1,
    };
    assert_eq!(changed, literal_count);
    Case {
        flavor,
        before,
        expected,
        groups,
        changed,
        refusal,
        required_patch,
    }
}

pub(crate) fn assert_complement(case: &Case) {
    let Some(expected) = &case.expected else {
        assert!(case.refusal.is_some());
        return;
    };
    assert_eq!(expected.len(), case.before.len());
    if case.flavor == Flavor::Int32 {
        for row in 0..3 {
            let (payload, _) = location(&case.before, Column::LargeInt32, row);
            assert_eq!(
                &case.before[payload + 4..payload + 8],
                &expected[payload + 4..payload + 8],
                "Original upper Int64 bytes remain opaque Int32 padding"
            );
        }
    }
    let mut payloads = Vec::new();
    let mut masks = BTreeMap::<usize, u8>::new();
    for group in &case.groups {
        for assignment in &group.updates {
            let (at, mask) = location(&case.before, group.column, assignment.row);
            payloads.push(at..at + group.column.width());
            *masks.entry(mask).or_default() |= 1 << (group.column.number() % 8);
            if assignment.replacement == Cell::Null || assignment.expected == assignment.replacement
            {
                assert_eq!(
                    &case.before[at..at + group.column.width()],
                    &expected[at..at + group.column.width()]
                );
            }
        }
    }
    for (mask, allowed) in &masks {
        assert_eq!((case.before[*mask] ^ expected[*mask]) & !allowed, 0);
    }
    for (at, (before, after)) in case.before.iter().zip(expected).enumerate() {
        assert!(
            before == after
                || masks.contains_key(&at)
                || payloads.iter().any(|span| span.contains(&at)),
            "unselected byte {at}"
        );
    }
    let (rebuilt, count) = reconstruct(&case.before, &case.groups);
    assert_eq!(&rebuilt, expected);
    assert_eq!(count, case.changed);
    let mut reversed = case.groups.clone();
    reversed.reverse();
    for group in &mut reversed {
        group.updates.reverse();
    }
    assert_eq!(reconstruct(&case.before, &reversed), (rebuilt, count));
    if count == 0 {
        assert_eq!(&case.before, expected);
    }
}
