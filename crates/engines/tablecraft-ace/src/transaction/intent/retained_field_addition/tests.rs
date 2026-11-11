//! Literal original requests and independent cold V3 reconstruction.
#[cfg(feature = "ace16-create")]
mod recovery;
#[cfg(feature = "ace16-create")]
mod wire;

use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::typed_fields::Kind;

fn literal(before: &[u8], kind: Kind) -> Vec<u8> {
    let (native, default_bytes, width, precision, scale, compression) = match kind {
        Kind::Boolean => (1, vec![0], 0_u16, 0, 0, false),
        Kind::Byte => (2, vec![0], 0, 0, 0, false),
        Kind::Int16 => (3, vec![0; 2], 0, 0, 0, false),
        Kind::Int32 => (4, vec![0; 4], 0, 0, 0, false),
        Kind::Int64 => (19, vec![0; 8], 0, 0, 0, false),
        Kind::Single => (6, vec![0; 4], 0, 0, 0, false),
        Kind::Double => (7, vec![0; 8], 0, 0, 0, false),
        Kind::Currency => (5, vec![0; 8], 0, 0, 0, false),
        Kind::DateTime => (8, Vec::new(), 0, 0, 0, false),
        Kind::Guid => (15, Vec::new(), 0, 0, 0, false),
        Kind::Decimal { precision, scale } => {
            let mut default = vec![0; 16];
            default.push(scale);
            (16, default, 0, precision, scale, false)
        }
        Kind::ShortText { units, compression } => (10, Vec::new(), units, 0, 0, compression),
    };
    let mut bytes = b"TCAC0012".to_vec();
    bytes.extend(((72 + 6 + 10 + default_bytes.len()) as u32).to_le_bytes());
    bytes.extend([12, 0, 0, 0]);
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    bytes.extend(hash.finalize());
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend(10_u16.to_le_bytes());
    bytes.extend([native, 0]);
    bytes.extend(width.to_le_bytes());
    bytes.extend([precision, scale, u8::from(compression), 0]);
    bytes.extend((default_bytes.len() as u16).to_le_bytes());
    bytes.extend([0; 2]);
    bytes.extend(b"Table1AddedValue");
    bytes.extend(default_bytes);
    bytes
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn typed_field_addition_original_is_not_an_older_journal_only_language() {
    use crate::{CurrentFieldAdditionIntentError, NativeCurrentFieldAdditionIntent};
    use tablecraft_core::limits::ResourceBudget;

    let wire = literal(&vec![0; 4096], Kind::Currency);
    assert!(matches!(
        NativeCurrentFieldAdditionIntent::decode_retained(
            &wire,
            &mut ResourceBudget::new(1_000_000, 1000, 1_000_000),
            &|| false,
        ),
        Err(CurrentFieldAdditionIntentError::InvalidRecord)
    ));
}
