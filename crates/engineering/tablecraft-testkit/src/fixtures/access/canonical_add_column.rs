//! Independent empty saved-table ADD-column SOURCE and SDK-observed oracle.
use super::{canonical_creation, canonical_native17_creation, put16, put32};

const PAGE: usize = 4096;

/// Complete original and expected images, authored without production codecs.
pub struct Source {
    /// Canonical empty keyless table with two ordinary Int32 fields.
    pub before: Vec<u8>,
    /// Complete expected image after the appended ordinary Int32 declaration.
    pub after: Vec<u8>,
}

/// Append a named third Int32 declaration using the independently observed layout.
///
/// The default `Extra` case is frozen against the reviewed Jackcess 5.0.4 public
/// ColumnBuilder.addToTable operation, not a TableCraft writer invocation.
pub fn source(name: &str) -> Source {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    assert_eq!(before.len(), 9 * PAGE);
    let original = &before[7 * PAGE..8 * PAGE];
    assert_eq!(&original[41..47], &[2, 0, 0, 0, 2, 0]);
    assert_eq!(&original[8..12], &133_u32.to_le_bytes());
    let mut definition = vec![0; PAGE];
    // Retain every existing header and field-descriptor byte independently of
    // the name-vector relocation caused by the new descriptor.
    definition[..113].copy_from_slice(&original[..113]);
    let mut entry = [0_u8; 25];
    entry[0] = 4;
    entry[1..5].copy_from_slice(&1625_u32.to_le_bytes());
    entry[5..7].copy_from_slice(&2_u16.to_le_bytes());
    entry[9..11].copy_from_slice(&2_u16.to_le_bytes());
    entry[15] = 3;
    entry[21..23].copy_from_slice(&8_u16.to_le_bytes());
    entry[23..25].copy_from_slice(&4_u16.to_le_bytes());
    definition[113..138].copy_from_slice(&entry);
    definition[138..156].copy_from_slice(&original[113..131]);
    let encoded = name.encode_utf16().collect::<Vec<_>>();
    put16(&mut definition, 156, (encoded.len() * 2) as u16);
    for (index, unit) in encoded.iter().enumerate() {
        put16(&mut definition, 158 + index * 2, *unit);
    }
    let end = 158 + encoded.len() * 2;
    put16(&mut definition, end, 65535);
    let length = end + 2;
    put16(&mut definition, 2, (PAGE - length - 8) as u16);
    put32(&mut definition, 8, length as u32);
    put16(&mut definition, 41, 3);
    put16(&mut definition, 45, 3);
    let mut after = before.clone();
    after[7 * PAGE..8 * PAGE].copy_from_slice(&definition);
    Source { before, after }
}
