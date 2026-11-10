//! Independent TCTD0003 bytes carry original identity, never replacement authority.
#[cfg(feature = "ace16-create")]
mod native_recovery;

use super::{NativeTableDropIntent, SourceProfile, TableDropIntentError};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn literal(before: &[u8], definition: u32, slot: u8, name: &str) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut bytes = b"TCTD0003".to_vec();
    bytes.extend((80_u32 + name.len() as u32).to_le_bytes());
    bytes.extend([1, 0, 0, 0]);
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend(digest.finalize());
    bytes.extend((definition as i32).to_le_bytes());
    bytes.extend(0x0f00_0001_i32.to_le_bytes());
    bytes.extend(definition.to_le_bytes());
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend([slot, 0, 0, 0]);
    bytes.extend((name.len() as u16).to_le_bytes());
    bytes.extend([0, 0]);
    bytes.extend(name.as_bytes());
    assert_eq!(bytes.len(), 80 + name.len());
    bytes
}

#[test]
fn retained_table_drop_wire_is_literal_bound_and_closed_from_old_families() {
    // Data-only wire validation deliberately uses an inert buffer, not a native
    // permission fixture. Native qualification is exercised separately.
    let before = vec![0; 14 * 4096];
    let encoded = literal(&before, 11, 8, "Added");
    let intent =
        NativeTableDropIntent::decode_retained(&encoded, &mut budget(), &|| false).unwrap();
    assert!(intent.is_retained());
    assert_eq!(intent.table(), "Added");
    intent
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), encoded);
    assert!(NativeTableDropIntent::decode(&encoded, &mut budget(), &|| false).is_err());
    assert!(NativeTableDropIntent::decode_current(&encoded, &mut budget(), &|| false).is_err());
    for magic in [b"TCTD0001", b"TCTD0002"] {
        let mut old = encoded.to_vec();
        old[..8].copy_from_slice(magic);
        assert!(NativeTableDropIntent::decode_retained(&old, &mut budget(), &|| false).is_err());
    }
    let mut changed = before;
    changed[4000] ^= 1;
    assert!(matches!(
        intent.verify_before(&changed, &mut budget(), &|| false),
        Err(TableDropIntentError::BeforeMismatch)
    ));
}

#[test]
fn retained_table_drop_wire_uses_native_geometry_without_legacy_image_quota() {
    let encoded = literal(&vec![0; 14 * 4096], 11, 8, "Added");
    for length in [64_u64 * 1024 * 1024 + 4096, 2_u64 * 1024 * 1024 * 1024] {
        let mut wide = encoded.to_vec();
        wide[16..24].copy_from_slice(&length.to_le_bytes());
        let intent =
            NativeTableDropIntent::decode_retained(&wide, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wide);
        for profile in [
            SourceProfile::Inline,
            SourceProfile::CreatorExternal,
            SourceProfile::Current,
        ] {
            let mut legacy =
                NativeTableDropIntent::decode_retained(&wide, &mut budget(), &|| false).unwrap();
            legacy.profile = profile;
            assert!(legacy.encode(&mut budget(), &|| false).is_err());
        }
    }
    for length in [0_u64, 8193, 2_u64 * 1024 * 1024 * 1024 + 4096] {
        let mut malformed = encoded.to_vec();
        malformed[16..24].copy_from_slice(&length.to_le_bytes());
        assert!(
            NativeTableDropIntent::decode_retained(&malformed, &mut budget(), &|| false).is_err()
        );
    }
}

#[test]
fn retained_table_drop_framing_cancellation_and_budget_errors_publish_no_request() {
    let encoded = literal(&vec![0; 14 * 4096], 11, 8, "Added");
    for end in 0..encoded.len() {
        assert!(
            NativeTableDropIntent::decode_retained(&encoded[..end], &mut budget(), &|| false)
                .is_err()
        );
    }
    for offset in [8, 12, 14, 15, 73, 74, 75, 76, 78, 79, 80] {
        let mut malformed = encoded.to_vec();
        malformed[offset] = 0xff;
        assert!(
            NativeTableDropIntent::decode_retained(&malformed, &mut budget(), &|| false).is_err()
        );
    }
    for offset in [56, 64, 68] {
        let mut malformed = encoded.to_vec();
        malformed[offset..offset + 4].fill(0);
        assert!(
            NativeTableDropIntent::decode_retained(&malformed, &mut budget(), &|| false).is_err()
        );
    }
    let mut suffix = encoded.to_vec();
    suffix.push(0);
    assert!(NativeTableDropIntent::decode_retained(&suffix, &mut budget(), &|| false).is_err());
    assert!(NativeTableDropIntent::decode_retained(&encoded, &mut budget(), &|| true).is_err());
    for mut allowance in [
        ResourceBudget::new(0, 1_000_000, 500_000_000),
        ResourceBudget::new(1_000_000_000, 1_000_000, 0),
    ] {
        assert!(
            NativeTableDropIntent::decode_retained(&encoded, &mut allowance, &|| false).is_err()
        );
    }
}
