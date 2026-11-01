//! Original-only nullable Int32 user requests beside native Int32 macro effects.
use super::canonical_mixed_before_change as original;
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

/// Separate original request contexts; neither consumes an earlier outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Activate,
    Clear,
}

impl Recipe {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Activate => "activate-id",
            Self::Clear => "clear-id",
        }
    }
}

/// Complete independently authored original and literal expected image.
pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
    pub expected_id: Option<i32>,
    pub replacement_id: Option<i32>,
    pub extra: Vec<u8>,
}

pub fn record(bytes: &[u8]) -> (usize, usize) {
    assert_eq!(bytes.len(), 9 * PAGE);
    let page = 8 * PAGE;
    assert_eq!(&bytes[page + 12..page + 14], &3_u16.to_le_bytes());
    let offset = u16::from_le_bytes(bytes[page + 14..page + 16].try_into().unwrap());
    assert_eq!(offset & 0xe000, 0);
    let start = page + usize::from(offset);
    assert_eq!(&bytes[start..start + 2], &6_u16.to_le_bytes());
    (start, 9 * PAGE)
}

/// Use only the old manual source BEFORE, original rows and actual XML bytes.
pub fn source(header: u8, recipe: Recipe) -> Case {
    let old = original::source(header, original::Recipe::VariableManual);
    let mut before = old.before;
    let mut rows = old.rows;
    let (start, end) = record(&before);
    assert_eq!((rows.len(), rows[0].len()), (3, 6));
    assert_eq!(rows[0][0], Value::Int32(1));
    assert_eq!(rows[0][5], Value::Int32(1000));
    assert_eq!(&before[start + 2..start + 6], &1_i32.to_le_bytes());
    assert_eq!(&before[start + 6..start + 10], &1000_i32.to_le_bytes());
    assert_eq!(before[end - 1] & 0x21, 0x21);

    let (expected_id, replacement_id) = match recipe {
        Recipe::Activate => {
            before[end - 1] &= !1;
            rows[0][0] = Value::Null;
            (None, Some(7))
        }
        Recipe::Clear => (Some(1), None),
    };
    let mut expected = before.clone();
    let mut final_rows = rows.clone();
    match recipe {
        Recipe::Activate => {
            expected[start + 2..start + 6].copy_from_slice(&7_i32.to_le_bytes());
            expected[start + 6..start + 10].copy_from_slice(&2007_i32.to_le_bytes());
            expected[end - 1] |= 1;
            final_rows[0][0] = Value::Int32(7);
            final_rows[0][5] = Value::Int32(2007);
        }
        Recipe::Clear => {
            // Current Score + NULL Id, then + Old.Score, both yield NULL.
            // Clearing each presence bit retains its original inactive payload.
            expected[end - 1] &= !0x21;
            final_rows[0][0] = Value::Null;
            final_rows[0][5] = Value::Null;
            assert_eq!(
                &expected[start + 2..start + 10],
                &before[start + 2..start + 10]
            );
        }
    }
    for (offset, (old, new)) in before.iter().zip(&expected).enumerate() {
        if !(start + 2..start + 10).contains(&offset) && offset != end - 1 {
            assert_eq!(old, new, "unrelated byte {offset}");
        }
    }
    assert_eq!(before[end - 1] & !0x21, expected[end - 1] & !0x21);

    Case {
        before,
        expected,
        rows,
        final_rows,
        expected_id,
        replacement_id,
        extra: old.extra,
    }
}
