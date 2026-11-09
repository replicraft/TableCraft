//! Literal same-width TDEF/live-MR2 pairs frozen before inert SDK observation.
//! Reuse the raw constructive source owners; no native parser/encoder participates.
pub mod fitting;

use super::canonical_created_number::mixed::after_off::{self, SourceCase};

/// Complete original and independently observed coordinated raw AFTER.
/// These fixed Number/Mixed sources select Score→Total or Label→Title only.
/// The exact spans come from docs/testing/saved_field_rename/model.py's frozen
/// first-party observations, not production constants or substring searching.
pub fn pair(case: SourceCase) -> [Vec<u8>; 2] {
    let before = after_off::source(case);
    let (spans, original, replacement): ([usize; 2], &[u8], &[u8]) = match case {
        SourceCase::Number => ([28805, 45027], b"S\0c\0o\0r\0e\0", b"T\0o\0t\0a\0l\0"),
        SourceCase::Mixed => ([28855, 44650], b"L\0a\0b\0e\0l\0", b"T\0i\0t\0l\0e\0"),
    };
    let mut after = before.clone();
    for start in spans {
        let end = start + original.len();
        assert_eq!(&before[start..end], original);
        after[start..end].copy_from_slice(replacement);
    }
    [before, after]
}

#[cfg(all(test, feature = "access-support"))]
mod tests;
