//! Literal indexed rename pairs, independent of native mutation/intent codecs.
use super::{
    canonical_catalog_names, canonical_native17_creation,
    canonical_ordinary_indexed::{self, Case},
    put32,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const USER_DEFINITION: usize = 7;
// Two Int32 fields: 75-byte indexed prefix, 50-byte descriptors, 6/12-byte names.
const PHYSICAL_INDEX: usize = 143;
const INDEX_MAP: usize = PHYSICAL_INDEX + 34;

/// Explicit original profile facts and independently constructed candidate.
pub struct Pair {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub indexes: Vec<&'static str>,
    pub index_pages: Vec<u32>,
}

/// Keep the raw indexed original, replacing only its independent catalog layout.
/// Both spellings are independently packed by the raw catalog fixture builder.
/// Unsupported raw recipes fail authoring assertions rather than repair bytes.
pub fn pair(header: u8, case: Case, old_name: &str, new_name: &str) -> Pair {
    assert!(matches!(header, 5 | 6));
    assert!(!matches!(
        case,
        Case::CounterOnly | Case::VariablePrimary | Case::RelocatedPrimary
    ));
    let original = canonical_ordinary_indexed::source(case);
    let mut before = canonical_native17_creation::source(original.bytes);
    let definition = USER_DEFINITION * PAGE;
    // Native17's seven catalog LVALs add six map records before the user index.
    // The original fixture's explicit physical map 16 becomes literal map 22.
    assert_eq!(
        &before[definition + INDEX_MAP..definition + INDEX_MAP + 4],
        &((3_u32 << 8) | 16).to_le_bytes()
    );
    put32(&mut before, definition + INDEX_MAP, (3 << 8) | 22);
    before[20] = header;

    let names = canonical_catalog_names::rename_pair(header, old_name, new_name, false);
    before[2 * PAGE..3 * PAGE].copy_from_slice(&names.before[2 * PAGE..3 * PAGE]);
    before[4 * PAGE..5 * PAGE].copy_from_slice(&names.before[4 * PAGE..5 * PAGE]);
    let mut after = before.clone();
    after[4 * PAGE..5 * PAGE].copy_from_slice(&names.after[4 * PAGE..5 * PAGE]);
    Pair {
        before,
        after,
        rows: original.rows,
        indexes: original.indexes,
        index_pages: original.index_pages,
    }
}
