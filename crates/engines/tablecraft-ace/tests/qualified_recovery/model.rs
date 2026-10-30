use sha2::{Digest, Sha256};
use std::ops::Range;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;
use tablecraft_testkit::access::secondary_index::{self, Population};

// Installation target: tests/qualified_recovery/model.rs. Reuse the original
// indexed literal owner once; do not copy its builder or create a fake module.
#[path = "../mutation/index_fixture.rs"]
mod index_fixture;

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Plain,
    NoOpPlain,
    WidePlain,
    Ascending,
    Descending,
    AscendingAlias,
    DescendingAlias,
    Node,
}

pub const SHAPES: [Shape; 8] = [
    Shape::Plain,
    Shape::NoOpPlain,
    Shape::WidePlain,
    Shape::Ascending,
    Shape::Descending,
    Shape::AscendingAlias,
    Shape::DescendingAlias,
    Shape::Node,
];

pub struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub value: i32,
    pub indexes: usize,
    pub descending: bool,
    pub changed_span: Range<usize>,
}

pub fn case(shape: Shape) -> Case {
    let (before, indexes, descending) = match shape {
        Shape::Plain | Shape::NoOpPlain | Shape::WidePlain => (fixture::database(), 0, false),
        Shape::Ascending => (index_fixture::database(false, false, false), 1, false),
        Shape::Descending => (index_fixture::database(true, false, false), 1, true),
        Shape::AscendingAlias => (index_fixture::database(false, true, false), 2, false),
        Shape::DescendingAlias => (index_fixture::database(true, true, false), 2, true),
        Shape::Node => (index_fixture::node_database(), 1, false),
    };
    let value: i32 = if matches!(shape, Shape::WidePlain) {
        0x12345678
    } else if matches!(shape, Shape::NoOpPlain) {
        10
    } else {
        12
    };
    let mut after = before.clone();
    // Independent literal Jet4 DATA slot0 and fixed field offset: count2,
    // Id[4], Score[4], nullmask. No producer offset/constants or write helper.
    let slot = u16::from_le_bytes([before[12 * 4096 + 14], before[12 * 4096 + 15]]) & 0x1fff;
    let start = 12 * 4096 + usize::from(slot) + 6;
    after[start..start + 4].copy_from_slice(&value.to_le_bytes());
    if indexes != 0 {
        // Literal wire key: nonNULL marker + biased BE i32 + logical ROW.
        // ASC leaf uses prefix4; DESC target is final entry's first suffix byte.
        // Node's first key lives in leaf15; separator20 is unchanged.
        let key_byte = if matches!(shape, Shape::Node) {
            15 * 4096 + 484
        } else if descending {
            13 * 4096 + 494
        } else {
            13 * 4096 + 484
        };
        after[key_byte] = if descending { 0xf3 } else { 0x0c };
    }
    Case {
        before,
        after,
        value,
        indexes,
        descending,
        changed_span: start..start + 4,
    }
}

pub fn real_ace(code: u8) -> Vec<u8> {
    secondary_index::source(code, Population::Distinct, false).before
}

pub fn unknown_property() -> Case {
    let before = fixture::with_rule("UnqualifiedJournalProperty", "opaque source data");
    let mut after = before.clone();
    let slot = u16::from_le_bytes([before[12 * 4096 + 14], before[12 * 4096 + 15]]) & 0x1fff;
    let start = 12 * 4096 + usize::from(slot) + 6;
    after[start..start + 4].copy_from_slice(&12_i32.to_le_bytes());
    Case {
        before,
        after,
        value: 12,
        indexes: 0,
        descending: false,
        changed_span: start..start + 4,
    }
}

#[derive(Clone, Copy)]
pub enum Marker {
    Pending,
    Partial,
    Committed,
}

pub fn journal(before: &[u8], after: &[u8], state: Marker) -> Vec<u8> {
    assert!(!before.is_empty());
    assert_eq!(before.len(), after.len());
    let mut prefix = b"TCIMG001".to_vec();
    prefix.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(&prefix);
    hash.update(before);
    hash.update(after);
    let digest = hash.finalize();
    let mut bytes = prefix;
    bytes.extend_from_slice(&digest);
    bytes.extend_from_slice(before);
    bytes.extend_from_slice(after);
    match state {
        Marker::Pending => {}
        Marker::Partial => bytes.extend_from_slice(b"COM"),
        Marker::Committed => {
            bytes.extend_from_slice(b"COMMIT01");
            bytes.extend_from_slice(&digest);
        }
    }
    bytes
}
