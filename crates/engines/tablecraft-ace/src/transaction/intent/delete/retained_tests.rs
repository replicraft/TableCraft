//! TCND3 retains ordered raw witnesses without inheriting legacy source quotas.
use super::{NativeDeleteIntent, NativeIntentError};
use crate::{EditError, RowDelete};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn literal(before: &[u8], table: &str, rows: &[(u64, &[u8])]) -> Vec<u8> {
    let length = 68
        + table.len()
        + rows
            .iter()
            .map(|(_, bytes)| 12 + bytes.len())
            .sum::<usize>();
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = b"TCND0003".to_vec();
    wire.extend((length as u32).to_le_bytes());
    wire.extend([3, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend((table.len() as u32).to_le_bytes());
    wire.extend((rows.len() as u32).to_le_bytes());
    wire.extend([0; 4]);
    wire.extend(table.as_bytes());
    for (ordinal, expected) in rows {
        wire.extend(ordinal.to_le_bytes());
        wire.extend((expected.len() as u32).to_le_bytes());
        wire.extend(*expected);
    }
    assert_eq!(wire.len(), length);
    wire
}

#[test]
fn retained_delete_matches_independent_wire_and_keeps_original_order_and_source_binding() {
    // Aligned bytes bind inert request data; no database or row authority is implied.
    let before = [0x35; 4096];
    let wire = literal(&before, "Items", &[(7, &[2, 0, 3]), (1, &[3, 0, 0, 7])]);
    let intent = NativeDeleteIntent::new_retained(
        "Items",
        vec![
            RowDelete::new(7, vec![2, 0, 3]),
            RowDelete::new(1, vec![3, 0, 0, 7]),
        ],
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
    let decoded = NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_retained());
    assert_eq!(decoded.table(), "Items");
    assert_eq!(decoded.deletes()[0].row(), 7);
    assert_eq!(decoded.deletes()[1].row(), 1);
    assert_eq!(decoded.deletes()[0].expected_bytes(), &[2, 0, 3]);
    assert_eq!(decoded.deletes()[1].expected_bytes(), &[3, 0, 0, 7]);
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    let mut changed = before;
    changed[100] ^= 1;
    assert!(matches!(
        decoded.verify_before(&changed, &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    assert!(NativeDeleteIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(NativeDeleteIntent::decode_current(&wire, &mut budget(), &|| false).is_err());
    for version in [1_u8, 2] {
        let mut older = wire.to_vec();
        older[7] = b'0' + version;
        older[12] = version;
        assert!(NativeDeleteIntent::decode_retained(&older, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn retained_delete_preflight_refuses_malformed_and_duplicate_witnesses_before_publication() {
    let before = [0; 4096];
    let wire = literal(&before, "Items", &[(7, &[2, 0, 3])]);
    for end in 0..wire.len() {
        let mut work = budget();
        let bytes = work.remaining(Resource::Bytes);
        assert!(NativeDeleteIntent::decode_retained(&wire[..end], &mut work, &|| false).is_err());
        assert_eq!(work.remaining(Resource::Bytes), bytes);
    }
    for offset in [8, 12, 14, 56, 60, 64, 68, 81] {
        let mut invalid = wire.to_vec();
        invalid[offset] ^= 0x80;
        if offset == 68 {
            invalid[offset] = 0xff;
        }
        assert!(
            NativeDeleteIntent::decode_retained(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    let duplicate = literal(&before, "Items", &[(7, &[2, 0, 3]), (7, &[3, 0, 0, 7])]);
    assert!(matches!(
        NativeDeleteIntent::decode_retained(&duplicate, &mut budget(), &|| false),
        Err(NativeIntentError::Request(EditError::Conflict))
    ));
    for length in [0, 2, 4097] {
        let payload = vec![0; length];
        let invalid = literal(&before, "Items", &[(0, &payload)]);
        assert!(NativeDeleteIntent::decode_retained(&invalid, &mut budget(), &|| false).is_err());
    }
    for mut work in [
        ResourceBudget::new(0, 0, 500_000_000),
        ResourceBudget::new(512 * 1024 * 1024, 0, 0),
    ] {
        assert!(NativeDeleteIntent::decode_retained(&wire, &mut work, &|| false).is_err());
    }
    assert!(NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| true).is_err());
}

#[test]
fn retained_delete_binding_and_names_use_native_and_wire_bounds_instead_of_old_caps() {
    let before = vec![0; 64 * 1024 * 1024 + 4096];
    let table = "x".repeat(8193);
    let intent =
        NativeDeleteIntent::new_retained(&table, Vec::new(), &before, &mut budget(), &|| false)
            .unwrap();
    let wire = intent.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(wire, literal(&before, &table, &[]));
    let decoded = NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(decoded.table(), table);
    assert!(decoded.deletes().is_empty());
    assert!(
        NativeDeleteIntent::new_current("Items", Vec::new(), &before, &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        NativeDeleteIntent::new_retained(
            "Items",
            Vec::new(),
            &before[..4095],
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for version in [1_u8, 2] {
        let mut older = wire.to_vec();
        older[7] = b'0' + version;
        older[12] = version;
        let result = if version == 1 {
            NativeDeleteIntent::decode(&older, &mut budget(), &|| false)
        } else {
            NativeDeleteIntent::decode_current(&older, &mut budget(), &|| false)
        };
        assert!(result.is_err());
    }
}

#[test]
fn retained_delete_sparse_ordinals_do_not_inherit_legacy_request_cardinality() {
    // Data-framing coverage does not qualify a table containing these records.
    let rows = (0..16_385)
        .map(|ordinal| RowDelete::new(16_384 + ordinal * 3, vec![2, 0, 1]))
        .collect();
    let intent =
        NativeDeleteIntent::new_retained("Items", rows, &[0; 4096], &mut budget(), &|| false)
            .unwrap();
    let wire = intent.encode(&mut budget(), &|| false).unwrap();
    let decoded = NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.deletes().len(), 16_385);
    assert_eq!(decoded.deletes().first().unwrap().row(), 16_384);
    assert_eq!(decoded.deletes().last().unwrap().row(), 65_536);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
}

#[cfg(feature = "ace16-create")]
mod recovery;
