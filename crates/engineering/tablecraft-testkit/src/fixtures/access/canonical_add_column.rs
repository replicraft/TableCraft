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
    let after = append(&before, name);
    Source { before, after }
}

/// Independently authored minimum one-field source, frozen against SDK addition.
pub fn one_column_source() -> Vec<u8> {
    let mut before = source("Extra").before;
    let original = &before[7 * PAGE..8 * PAGE];
    let mut definition = vec![0; PAGE];
    definition[..88].copy_from_slice(&original[..88]);
    definition[88..94].copy_from_slice(&original[113..119]);
    put16(&mut definition, 94, 65535);
    put16(&mut definition, 2, 3992);
    put32(&mut definition, 8, 96);
    put16(&mut definition, 41, 1);
    put16(&mut definition, 45, 1);
    before[7 * PAGE..8 * PAGE].copy_from_slice(&definition);
    before
}

/// Build an independent expected appended declaration for another raw empty source.
/// This helper parses only its literal one-page field/name geometry, not engine APIs.
pub fn append(before: &[u8], name: &str) -> Vec<u8> {
    assert_eq!(before.len(), 9 * PAGE);
    let original = &before[7 * PAGE..8 * PAGE];
    assert_eq!(&original[16..20], &[0; 4]);
    let count = u16::from_le_bytes(original[41..43].try_into().unwrap()) as usize;
    assert!((1..16).contains(&count));
    assert_eq!(&original[43..45], &[0; 2]);
    assert_eq!(&original[45..47], &(count as u16).to_le_bytes());
    let old_end = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    let old_names = 63 + count * 25;
    let new_names = old_names + 25;
    let name_bytes = old_end - 2 - old_names;
    assert_eq!(&original[old_end - 2..old_end], &[255, 255]);
    let mut definition = vec![0; PAGE];
    // Existing descriptors retain their identities and fixed data offsets.
    definition[..old_names].copy_from_slice(&original[..old_names]);
    let mut entry = [0_u8; 25];
    entry[0] = 4;
    entry[1..5].copy_from_slice(&1625_u32.to_le_bytes());
    entry[5..7].copy_from_slice(&(count as u16).to_le_bytes());
    entry[9..11].copy_from_slice(&(count as u16).to_le_bytes());
    entry[15] = 3;
    entry[21..23].copy_from_slice(&((count * 4) as u16).to_le_bytes());
    entry[23..25].copy_from_slice(&4_u16.to_le_bytes());
    definition[old_names..new_names].copy_from_slice(&entry);
    definition[new_names..new_names + name_bytes]
        .copy_from_slice(&original[old_names..old_end - 2]);
    let encoded = name.encode_utf16().collect::<Vec<_>>();
    let appended_name = new_names + name_bytes;
    put16(&mut definition, appended_name, (encoded.len() * 2) as u16);
    for (index, unit) in encoded.iter().enumerate() {
        put16(&mut definition, appended_name + 2 + index * 2, *unit);
    }
    let end = appended_name + 2 + encoded.len() * 2;
    put16(&mut definition, end, 65535);
    let length = end + 2;
    put16(&mut definition, 2, (PAGE - length - 8) as u16);
    put32(&mut definition, 8, length as u32);
    put16(&mut definition, 41, (count + 1) as u16);
    put16(&mut definition, 45, (count + 1) as u16);
    let mut after = before.to_vec();
    after[7 * PAGE..8 * PAGE].copy_from_slice(&definition);
    after
}
