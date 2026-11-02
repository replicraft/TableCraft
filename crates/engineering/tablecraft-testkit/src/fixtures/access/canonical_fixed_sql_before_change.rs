//! Separate discriminator originals and literal normal SQL fixed outcomes.
use super::{canonical_fixed_before_change as fixed, canonical_mixed_before_change as original};
use tablecraft_core::values::Value;

use fixed::{Cell, Group, Update};

/// Literal user targets, separate from the macro-only Int32 field3.
pub const FIELDS: [(usize, &str, usize, usize); 5] = [
    (0, "Flag", 0, 0),
    (3, "Int32Value", 3, 4),
    (4, "Int64Value", 7, 8),
    (5, "Money", 15, 8),
    (8, "DateValue", 35, 8),
];

/// SQL expressions whose result carriers are explicit and independent of rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Boolean,
    Int64,
    Currency,
    Date,
    Joint,
}

/// All separate SQL-addressable request contexts.
pub const RECIPES: [Recipe; 5] = [
    Recipe::Boolean,
    Recipe::Int64,
    Recipe::Currency,
    Recipe::Date,
    Recipe::Joint,
];

impl Recipe {
    /// Stable exported artifact spelling.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Int64 => "int64",
            Self::Currency => "currency",
            Self::Date => "date",
            Self::Joint => "joint-original-rhs",
        }
    }

    /// Literal closed local SQL; no source value is interpolated into syntax.
    pub const fn sql(self) -> &'static str {
        match self {
            Self::Boolean => "UPDATE Items SET Flag=True WHERE Int32Value=0",
            Self::Int64 => "UPDATE Items SET Int64Value=CLngLng(-101) WHERE Int32Value=0",
            Self::Currency => "UPDATE Items SET Money=CCur(-1.0001) WHERE Int32Value=0",
            Self::Date => "UPDATE Items SET DateValue=#1899-12-31 06:00:00# WHERE Int32Value=0",
            Self::Joint => {
                "UPDATE Items SET Flag=True,Int64Value=CLngLng(IIf([Flag],7,42)),Money=CCur(-1.0001),DateValue=#1899-12-31 06:00:00# WHERE Int32Value=0"
            }
        }
    }

    /// Reverse SET order proves every RHS still reads the original false Flag.
    pub const fn reversed_sql(self) -> &'static str {
        match self {
            Self::Joint => {
                "UPDATE Items SET DateValue=#1899-12-31 06:00:00#,Money=CCur(-1.0001),Int64Value=CLngLng(IIf([Flag],7,42)),Flag=True WHERE Int32Value=0"
            }
            _ => self.sql(),
        }
    }
}

/// Whole images and original requests, authored before any new SQL execution.
pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
    pub groups: Vec<Group>,
    pub extra: Vec<u8>,
    pub sql: &'static str,
    pub reversed_sql: &'static str,
}

/// Independent bounded literal record framing; no production decoder or encoder.
pub fn record(bytes: &[u8], ordinal: usize) -> (usize, usize) {
    assert_eq!(bytes.len(), 9 * 4096);
    assert!(ordinal < 2);
    let page = 8 * 4096;
    assert_eq!(&bytes[page + 12..page + 14], &2_u16.to_le_bytes());
    let word = page + 14 + ordinal * 2;
    let offset = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    assert_eq!(offset & 0xe000, 0);
    let start = page + usize::from(offset);
    let end = if ordinal == 0 {
        9 * 4096
    } else {
        page + usize::from(u16::from_le_bytes(
            bytes[word - 2..word].try_into().unwrap(),
        ))
    };
    assert_eq!(end - start, 63);
    assert_eq!(&bytes[start..start + 2], &10_u16.to_le_bytes());
    (start, end)
}

fn group(column: usize, replacement: Cell) -> Group {
    let expected = match column {
        0 => Cell::Boolean(false),
        4 => Cell::Int64(0),
        5 => Cell::Currency(1),
        8 => Cell::Date(0),
        _ => unreachable!(),
    };
    Group {
        column,
        updates: vec![Update {
            ordinal: 0,
            expected,
            replacement,
        }],
    }
}

fn overlay(bytes: &mut [u8], column: usize, cell: Cell) {
    let (start, end) = record(bytes, 0);
    let (_, _, offset, width) = FIELDS.iter().find(|field| field.0 == column).unwrap();
    if let Cell::Boolean(value) = cell {
        assert_eq!(column, 0);
        bytes[end - 2] = (bytes[end - 2] & !1) | u8::from(value);
        return;
    }
    let encoded = match cell {
        Cell::Int64(value) | Cell::Currency(value) => value.to_le_bytes(),
        Cell::Date(bits) => bits.to_le_bytes(),
        _ => unreachable!(),
    };
    assert_eq!(*width, 8);
    assert_ne!(bytes[end - 2 + column / 8] & (1 << (column % 8)), 0);
    bytes[start + 2 + offset..start + 2 + offset + width].copy_from_slice(&encoded);
}

/// Consume ONLY a qualified old BEFORE/rows/program, never its prior outcome.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let old = original::source(header, original::Recipe::FixedSequential);
    let mut before = old.before;
    let original_before = before.clone();
    let mut rows = old.rows;
    assert_eq!((rows.len(), rows[0].len()), (2, 10));
    assert_eq!(rows[0][3], Value::Int32(0));
    assert_eq!(rows[1][3], Value::Int32(0));
    let (second, _) = record(&before, 1);
    before[second + 5..second + 9].copy_from_slice(&10_i32.to_le_bytes());
    rows[1][3] = Value::Int32(10);
    for (at, (old, new)) in original_before.iter().zip(&before).enumerate() {
        if !(second + 5..second + 9).contains(&at) {
            assert_eq!(old, new, "original discriminator complement byte {at}");
        }
    }
    let groups = match recipe {
        Recipe::Boolean => vec![group(0, Cell::Boolean(true))],
        Recipe::Int64 => vec![group(4, Cell::Int64(-101))],
        Recipe::Currency => vec![group(5, Cell::Currency(-10_001))],
        Recipe::Date => vec![group(8, Cell::Date(0x3ff4_0000_0000_0000))],
        Recipe::Joint => vec![
            group(0, Cell::Boolean(true)),
            group(4, Cell::Int64(42)),
            group(5, Cell::Currency(-10_001)),
            group(8, Cell::Date(0x3ff4_0000_0000_0000)),
        ],
    };
    let mut expected = before.clone();
    let mut final_rows = rows.clone();
    for target in &groups {
        let update = &target.updates[0];
        assert_eq!(rows[0][target.column], update.expected.value());
        overlay(&mut expected, target.column, update.replacement);
        final_rows[0][target.column] = update.replacement.value();
    }
    // The stored sequential XML sees unchanged I32 Current=0/Old=0, then +1.
    let (first, _) = record(&before, 0);
    expected[first + 5..first + 9].copy_from_slice(&1_i32.to_le_bytes());
    final_rows[0][3] = Value::Int32(1);
    let mut allowed = vec![false; before.len()];
    allowed[first + 5..first + 9].fill(true);
    for target in &groups {
        let (_, _, offset, width) = FIELDS
            .iter()
            .find(|field| field.0 == target.column)
            .unwrap();
        allowed[first + 2 + offset..first + 2 + offset + width].fill(true);
        if target.column == 0 {
            let (_, end) = record(&before, 0);
            allowed[end - 2] = true;
            assert_eq!(before[end - 2] & !1, expected[end - 2] & !1);
        }
    }
    for (at, (old, new)) in before.iter().zip(&expected).enumerate() {
        if !allowed[at] {
            assert_eq!(old, new, "expected complete complement byte {at}");
        }
    }
    let (second, second_end) = record(&before, 1);
    assert_eq!(&before[second..second_end], &expected[second..second_end]);
    assert_eq!(final_rows[1], rows[1]);
    Case {
        before,
        expected,
        rows,
        final_rows,
        groups,
        extra: old.extra,
        sql: recipe.sql(),
        reversed_sql: recipe.reversed_sql(),
    }
}
