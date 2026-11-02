//! Separate present-Id source companion; existing nullable sources unchanged.
use super::{
    canonical_mixed_before_change::Update, canonical_nullable_index_before_change as original,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

/// Original ordinary nullable Id index and literal Id/Score final image.
pub struct Case {
    /// Exact old compatibility BEFORE, never a prior expected/candidate image.
    pub before: Vec<u8>,
    /// Complete intended result with two fixed fields and one leaf body changed.
    pub expected: Vec<u8>,
    /// Original complete scalar rows.
    pub rows: Vec<Vec<Value>>,
    /// Literal final rows, including unselected logical NULL Id.
    pub final_rows: Vec<Vec<Value>>,
    /// The original user target; macro-written Score is not a user request.
    pub column: &'static str,
    /// Original present Id1→7 request, without macro-final assignments.
    pub update: Update,
}

fn location(bytes: &[u8], ordinal: usize) -> (usize, usize) {
    let word = 8 * PAGE + 14 + 2 * ordinal;
    let start = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    assert_eq!(start & 0xe000, 0);
    let end = if ordinal == 0 {
        9 * PAGE
    } else {
        8 * PAGE
            + usize::from(u16::from_le_bytes(
                bytes[word - 2..word].try_into().unwrap(),
            ))
    };
    (8 * PAGE + usize::from(start), end)
}

/// Construct only from the old original and literal Current/Old arithmetic.
pub fn source(header: u8) -> Case {
    let old = original::source(header);
    let before = old.before;
    assert_eq!(before.len(), 10 * PAGE);
    assert_eq!(old.rows[0][0], Value::Int32(1));
    assert_eq!(old.rows[0][5], Value::Int32(1000));
    assert_eq!(old.rows[2][0], Value::Null);
    let mut expected = before.clone();
    let (start, _) = location(&before, 0);
    assert_eq!(&before[start..start + 2], &6_u16.to_le_bytes());
    assert_eq!(&before[start + 2..start + 6], &1_i32.to_le_bytes());
    assert_eq!(&before[start + 6..start + 10], &1000_i32.to_le_bytes());
    expected[start + 2..start + 6].copy_from_slice(&7_i32.to_le_bytes());
    // Stored statements: Score := CurrentScore + CurrentId; then + Old.Score.
    // Original user Id7 therefore produces 1000 + 7 + 1000 = 2007.
    expected[start + 6..start + 10].copy_from_slice(&2007_i32.to_le_bytes());
    let leaf = 9 * PAGE;
    assert_eq!(&before[leaf..leaf + 2], &[4, 1]);
    assert_eq!(&before[leaf + 2..leaf + 4], &3593_u16.to_le_bytes());
    assert_eq!(&before[leaf + 24..leaf + 26], &0_u16.to_le_bytes());
    let old_entries = [
        0, 0, 0, 8, 2, 0x7f, 0x80, 0, 0, 1, 0, 0, 8, 0, 0x7f, 0x80, 0, 0, 2, 0, 0, 8, 1,
    ];
    assert_eq!(&before[leaf + 480..leaf + 503], &old_entries);
    let final_entries = [
        0, 0, 0, 8, 2, 0x7f, 0x80, 0, 0, 2, 0, 0, 8, 1, 0x7f, 0x80, 0, 0, 7, 0, 0, 8, 0,
    ];
    expected[leaf + 480..leaf + 503].copy_from_slice(&final_entries);
    assert_eq!(&expected[..start + 2], &before[..start + 2]);
    assert_eq!(
        &expected[start + 10..leaf + 480],
        &before[start + 10..leaf + 480]
    );
    // The old source has retained bytes after the active23B span; do not zero
    // or replace that tail with an assumed padding constant.
    assert_eq!(&expected[leaf + 503..], &before[leaf + 503..]);
    let (null, end) = location(&before, 2);
    assert_eq!(&before[null + 2..null + 6], &3_i32.to_le_bytes());
    assert_eq!(before[end - 1] & 1, 0);
    assert_eq!(&expected[null..end], &before[null..end]);
    let mut final_rows = old.rows.clone();
    final_rows[0][0] = Value::Int32(7);
    final_rows[0][5] = Value::Int32(2007);
    Case {
        before,
        expected,
        rows: old.rows,
        final_rows,
        column: "Id",
        update: Update {
            ordinal: 0,
            expected: 1,
            replacement: 7,
        },
    }
}
