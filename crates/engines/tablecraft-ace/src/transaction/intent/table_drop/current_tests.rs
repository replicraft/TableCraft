//! Current table DROP wire data retains its own closed source family.
#[cfg(feature = "ace16-create")]
mod native_recovery;

use super::{NativeTableDropIntent, SourceProfile, TableDropIntentError};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn literal(before: &[u8]) -> Vec<u8> {
    // Independent 80-byte header and original logical catalog-root identity.
    // Stored caller scope and replacement bytes are deliberately absent.
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);

    let mut bytes = b"TCTD0002".to_vec();
    bytes.extend(86_u32.to_le_bytes());
    bytes.extend([1, 0, 0, 0]);
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend(digest.finalize());
    bytes.extend(7_i32.to_le_bytes());
    bytes.extend(0x0f00_0001_i32.to_le_bytes());
    bytes.extend(7_u32.to_le_bytes());
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend([7, 0, 0, 0]);
    bytes.extend([6, 0, 0, 0]);
    bytes.extend(b"Table1");
    assert_eq!(bytes.len(), 86);
    bytes
}

#[test]
fn current_table_drop_wire_preserves_binding_and_keeps_original_decoder_closed() {
    // Valid wire binding is intentionally not a native database or permission.
    let before = vec![0; 11 * 4096];
    let encoded = literal(&before);
    let intent = NativeTableDropIntent::decode_current(&encoded, &mut budget(), &|| false).unwrap();
    assert!(matches!(intent.profile, SourceProfile::Current));
    assert_eq!(intent.table(), "Table1");
    intent
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), encoded);
    assert!(NativeTableDropIntent::decode(&encoded, &mut budget(), &|| false).is_err());

    for profile in [1_u16, 2] {
        let mut old = encoded.to_vec();
        old[..8].copy_from_slice(b"TCTD0001");
        old[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(NativeTableDropIntent::decode_current(&old, &mut budget(), &|| false).is_err());
        let old_intent = NativeTableDropIntent::decode(&old, &mut budget(), &|| false).unwrap();
        assert_eq!(old_intent.encode(&mut budget(), &|| false).unwrap(), old);
    }

    let mut changed = before;
    changed[4096 + 3000] = 1;
    assert!(matches!(
        intent.verify_before(&changed, &mut budget(), &|| false),
        Err(TableDropIntentError::BeforeMismatch)
    ));
}

#[test]
fn current_table_drop_malformed_wire_and_exhausted_resources_never_publish_intents() {
    let encoded = literal(&vec![0; 11 * 4096]);
    for length in 0..encoded.len() {
        assert!(
            NativeTableDropIntent::decode_current(&encoded[..length], &mut budget(), &|| false,)
                .is_err()
        );
    }

    for offset in [8, 12, 14, 73, 76, 78, 80] {
        let mut malformed = encoded.to_vec();
        malformed[offset] = 0xff;
        assert!(
            NativeTableDropIntent::decode_current(&malformed, &mut budget(), &|| false).is_err()
        );
    }
    for offset in [64, 68] {
        let mut malformed = encoded.to_vec();
        malformed[offset..offset + 4].fill(0);
        assert!(
            NativeTableDropIntent::decode_current(&malformed, &mut budget(), &|| false).is_err()
        );
    }
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(512 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            NativeTableDropIntent::decode_current(&encoded, &mut allowance, &|| false).is_err()
        );
    }
    assert!(NativeTableDropIntent::decode_current(&encoded, &mut budget(), &|| true).is_err());
}
