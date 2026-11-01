//! Original-only wide user intents and literal Int32 BeforeChange outcomes.
use super::canonical_mixed_before_change as original;
use tablecraft_core::values::{Currency, LegacyDateTime, Value};

const PAGE: usize = 4096;

/// Stable literal source/request recipe, not an observed writer result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    FlagChange,
    Int64Minimum,
    CurrencyMaximum,
    DateSignedZero,
    DateTemporalAlias,
    Int64NullToValue,
    Int64ValueToNull,
    CurrencyNullToSameBytes,
    DateValueToNull,
    InactiveDateSibling,
    JointSharedMask,
    DisjointRows,
    RestoreAndWide,
    EntirelyEmpty,
    EqualDate,
    EqualInactiveDate,
    WrongCarrier,
    StaleLate,
    DuplicateColumn,
}

/// Every recipe, including intended refusal contexts.
pub const RECIPES: [Recipe; 19] = [
    Recipe::FlagChange,
    Recipe::Int64Minimum,
    Recipe::CurrencyMaximum,
    Recipe::DateSignedZero,
    Recipe::DateTemporalAlias,
    Recipe::Int64NullToValue,
    Recipe::Int64ValueToNull,
    Recipe::CurrencyNullToSameBytes,
    Recipe::DateValueToNull,
    Recipe::InactiveDateSibling,
    Recipe::JointSharedMask,
    Recipe::DisjointRows,
    Recipe::RestoreAndWide,
    Recipe::EntirelyEmpty,
    Recipe::EqualDate,
    Recipe::EqualInactiveDate,
    Recipe::WrongCarrier,
    Recipe::StaleLate,
    Recipe::DuplicateColumn,
];

impl Recipe {
    /// Stable artifact spelling.
    pub const fn id(self) -> &'static str {
        match self {
            Self::FlagChange => "flag-change",
            Self::Int64Minimum => "int64-minimum",
            Self::CurrencyMaximum => "currency-maximum",
            Self::DateSignedZero => "date-signed-zero",
            Self::DateTemporalAlias => "date-temporal-alias",
            Self::Int64NullToValue => "int64-null-to-value",
            Self::Int64ValueToNull => "int64-value-to-null",
            Self::CurrencyNullToSameBytes => "currency-null-to-same-bytes",
            Self::DateValueToNull => "date-value-to-null",
            Self::InactiveDateSibling => "inactive-date-sibling",
            Self::JointSharedMask => "joint-shared-mask",
            Self::DisjointRows => "disjoint-rows",
            Self::RestoreAndWide => "restore-and-wide",
            Self::EntirelyEmpty => "entirely-empty",
            Self::EqualDate => "equal-date",
            Self::EqualInactiveDate => "equal-inactive-date",
            Self::WrongCarrier => "wrong-carrier",
            Self::StaleLate => "stale-late",
            Self::DuplicateColumn => "duplicate-column",
        }
    }
}

/// Literal carrier with its own independent payload law.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Null,
    Boolean(bool),
    Int32(i32),
    Int64(i64),
    Currency(i64),
    Date(u64),
}

impl Cell {
    /// Actual core carrier, without numeric or temporal normalization.
    pub fn value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Boolean(value) => Value::Boolean(value),
            Self::Int32(value) => Value::Int32(value),
            Self::Int64(value) => Value::Int64(value),
            Self::Currency(value) => Value::Currency(Currency::from_scaled(value)),
            Self::Date(bits) => Value::DateTime(LegacyDateTime::from_bits(bits).unwrap()),
        }
    }
}

/// Original user assignment; macro-only writes are deliberately absent.
pub struct Update {
    pub ordinal: usize,
    pub expected: Cell,
    pub replacement: Cell,
}

/// Explicit target group, including real empty declarations.
pub struct Group {
    pub column: usize,
    pub updates: Vec<Update>,
}

/// Intended future normal refusal; source readability remains separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    EventTiming,
    Carrier,
    Conflict,
    DuplicateColumn,
}

/// Complete source and independently authored whole-image expectation.
pub struct Case {
    pub before: Vec<u8>,
    pub expected: Option<Vec<u8>>,
    pub rows: Vec<Vec<Value>>,
    pub final_rows: Option<Vec<Vec<Value>>>,
    pub groups: Vec<Group>,
    pub extra: Vec<u8>,
    pub refusal: Option<Refusal>,
    pub event_rows: Vec<usize>,
    pub changed_rows: usize,
}

/// Literal actual field-number/name/offset/width facts of the old fixed owner.
pub const FIELDS: [(usize, &str, usize, usize); 5] = [
    (0, "Flag", 0, 0),
    (3, "Int32Value", 3, 4),
    (4, "Int64Value", 7, 8),
    (5, "Money", 15, 8),
    (8, "DateValue", 35, 8),
];

/// Qualified fixed source has two 63-byte records; no production layout reader.
pub fn record(bytes: &[u8], ordinal: usize) -> (usize, usize) {
    assert!(ordinal < 2);
    let page = 8 * PAGE;
    assert_eq!(&bytes[page + 12..page + 14], &2_u16.to_le_bytes());
    let word = page + 14 + 2 * ordinal;
    let raw = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    assert_eq!(raw & 0xe000, 0);
    let start = page + usize::from(raw);
    let end = if ordinal == 0 {
        9 * PAGE
    } else {
        page + usize::from(u16::from_le_bytes(
            bytes[word - 2..word].try_into().unwrap(),
        ))
    };
    assert_eq!(end - start, 63);
    assert_eq!(&bytes[start..start + 2], &10_u16.to_le_bytes());
    (start, end)
}

fn overlay(bytes: &mut [u8], ordinal: usize, column: usize, value: Cell) {
    let (_, _, offset, width) = FIELDS.iter().find(|field| field.0 == column).unwrap();
    let (start, end) = record(bytes, ordinal);
    let mask = end - 2 + column / 8;
    let bit = 1 << (column % 8);
    if let Cell::Boolean(value) = value {
        assert_eq!((column, *width), (0, 0));
        bytes[mask] = (bytes[mask] & !bit) | if value { bit } else { 0 };
        return;
    }
    if value == Cell::Null {
        bytes[mask] &= !bit;
        return;
    }
    bytes[mask] |= bit;
    let at = start + 2 + offset;
    let payload = match value {
        Cell::Int32(value) => value.to_le_bytes().to_vec(),
        Cell::Int64(value) | Cell::Currency(value) => value.to_le_bytes().to_vec(),
        Cell::Date(bits) => bits.to_le_bytes().to_vec(),
        Cell::Null | Cell::Boolean(_) => unreachable!(),
    };
    assert_eq!(payload.len(), *width);
    bytes[at..at + width].copy_from_slice(&payload);
}

fn group(column: usize, ordinal: usize, expected: Cell, replacement: Cell) -> Group {
    Group {
        column,
        updates: vec![Update {
            ordinal,
            expected,
            replacement,
        }],
    }
}

/// Build solely from the old BEFORE and literal original-request arithmetic.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let old = original::source(
        header,
        if recipe == Recipe::RestoreAndWide {
            original::Recipe::FixedRestore
        } else {
            original::Recipe::FixedSequential
        },
    );
    // No old expected/final rows or user requests are consumed as authority.
    let mut before = old.before;
    let mut rows = old.rows;
    assert_eq!(rows.len(), 2);
    for (ordinal, row) in rows.iter().enumerate() {
        record(&before, ordinal);
        assert_eq!(row[0], Cell::Boolean(false).value());
        assert_eq!(row[3], Cell::Int32(0).value());
        assert_eq!(row[4], Cell::Int64(0).value());
        assert_eq!(row[5], Cell::Currency(1).value());
        assert_eq!(row[8], Cell::Date((ordinal as u64) << 63).value());
    }
    let mut original_cell = |ordinal, column, cell| {
        overlay(&mut before, ordinal, column, cell);
        rows[ordinal][column] = cell.value();
    };
    match recipe {
        Recipe::DateTemporalAlias => original_cell(0, 8, Cell::Date(0x3fd0_0000_0000_0000)),
        Recipe::Int64NullToValue => original_cell(0, 4, Cell::Null),
        Recipe::CurrencyNullToSameBytes => original_cell(0, 5, Cell::Null),
        Recipe::InactiveDateSibling | Recipe::EqualInactiveDate => {
            original_cell(1, 8, Cell::Null);
            let (start, _) = record(&before, 1);
            before[start + 37..start + 45].fill(0xff);
        }
        _ => {}
    }

    let mut groups = match recipe {
        Recipe::FlagChange | Recipe::InactiveDateSibling => {
            vec![group(0, 0, Cell::Boolean(false), Cell::Boolean(true))]
        }
        Recipe::Int64Minimum => vec![group(4, 0, Cell::Int64(0), Cell::Int64(i64::MIN))],
        Recipe::CurrencyMaximum => {
            vec![group(5, 0, Cell::Currency(1), Cell::Currency(i64::MAX))]
        }
        Recipe::DateSignedZero => {
            vec![group(
                8,
                0,
                Cell::Date(0),
                Cell::Date(0x8000_0000_0000_0000),
            )]
        }
        Recipe::DateTemporalAlias => vec![group(
            8,
            0,
            Cell::Date(0x3fd0_0000_0000_0000),
            Cell::Date(0xbfd0_0000_0000_0000),
        )],
        Recipe::Int64NullToValue => vec![group(4, 0, Cell::Null, Cell::Int64(42))],
        Recipe::Int64ValueToNull => vec![group(4, 0, Cell::Int64(0), Cell::Null)],
        Recipe::CurrencyNullToSameBytes => vec![group(5, 0, Cell::Null, Cell::Currency(1))],
        Recipe::DateValueToNull => vec![group(8, 0, Cell::Date(0), Cell::Null)],
        Recipe::JointSharedMask => vec![
            group(0, 0, Cell::Boolean(false), Cell::Boolean(true)),
            group(4, 0, Cell::Int64(0), Cell::Null),
            group(5, 0, Cell::Currency(1), Cell::Currency(-10_001)),
            group(8, 0, Cell::Date(0), Cell::Date(0x3fe0_0000_0000_0000)),
        ],
        Recipe::DisjointRows => vec![
            group(0, 0, Cell::Boolean(false), Cell::Boolean(true)),
            group(5, 1, Cell::Currency(1), Cell::Currency(i64::MIN)),
        ],
        Recipe::RestoreAndWide => {
            vec![group(5, 0, Cell::Currency(1), Cell::Currency(-1))]
        }
        Recipe::EntirelyEmpty => FIELDS
            .iter()
            .map(|field| Group {
                column: field.0,
                updates: Vec::new(),
            })
            .collect(),
        Recipe::EqualDate => vec![group(
            8,
            1,
            Cell::Date(0x8000_0000_0000_0000),
            Cell::Date(0x8000_0000_0000_0000),
        )],
        Recipe::EqualInactiveDate => vec![group(8, 1, Cell::Null, Cell::Null)],
        Recipe::WrongCarrier => vec![group(4, 0, Cell::Currency(0), Cell::Currency(42))],
        Recipe::StaleLate => vec![
            group(0, 0, Cell::Boolean(false), Cell::Boolean(true)),
            group(4, 1, Cell::Int64(1), Cell::Int64(42)),
        ],
        Recipe::DuplicateColumn => vec![
            group(0, 0, Cell::Boolean(false), Cell::Boolean(true)),
            group(0, 1, Cell::Boolean(false), Cell::Boolean(true)),
        ],
    };
    let refusal = match recipe {
        Recipe::EqualDate | Recipe::EqualInactiveDate => Some(Refusal::EventTiming),
        Recipe::WrongCarrier => Some(Refusal::Carrier),
        Recipe::StaleLate => Some(Refusal::Conflict),
        Recipe::DuplicateColumn => Some(Refusal::DuplicateColumn),
        _ => None,
    };
    let mut event_rows = Vec::new();
    for ordinal in 0..2 {
        if groups
            .iter()
            .any(|group| group.updates.iter().any(|update| update.ordinal == ordinal))
        {
            event_rows.push(ordinal);
        }
    }
    let (expected, final_rows, changed_rows) = if refusal.is_none() {
        let mut expected = before.clone();
        let mut final_rows = rows.clone();
        for group in &groups {
            for update in &group.updates {
                assert_eq!(rows[update.ordinal][group.column], update.expected.value());
                overlay(
                    &mut expected,
                    update.ordinal,
                    group.column,
                    update.replacement,
                );
                final_rows[update.ordinal][group.column] = update.replacement.value();
            }
        }
        for &ordinal in &event_rows {
            // Sequential XML reads Int32 Current=0, Old=0, then increments by1.
            // Restore XML assigns Old=0, irrespective of the wide user change.
            let value = if recipe == Recipe::RestoreAndWide {
                0
            } else {
                1
            };
            overlay(&mut expected, ordinal, 3, Cell::Int32(value));
            final_rows[ordinal][3] = Cell::Int32(value).value();
        }
        let changed_rows = (0..2)
            .filter(|&ordinal| {
                let (start, end) = record(&before, ordinal);
                before[start..end] != expected[start..end]
            })
            .count();
        // Independently reverse group order; mask accumulation cannot depend
        // on user ordering, and each row's event still executes exactly once.
        groups.reverse();
        let mut reversed = before.clone();
        for group in &groups {
            for update in &group.updates {
                overlay(
                    &mut reversed,
                    update.ordinal,
                    group.column,
                    update.replacement,
                );
            }
        }
        for &ordinal in &event_rows {
            let value = if recipe == Recipe::RestoreAndWide {
                0
            } else {
                1
            };
            overlay(&mut reversed, ordinal, 3, Cell::Int32(value));
        }
        groups.reverse();
        assert_eq!(reversed, expected);
        (Some(expected), Some(final_rows), changed_rows)
    } else {
        (None, None, 0)
    };
    Case {
        before,
        expected,
        rows,
        final_rows,
        groups,
        extra: old.extra,
        refusal,
        event_rows,
        changed_rows,
    }
}
