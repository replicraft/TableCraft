//! Opaque-history presence is original data, not permission to reuse TCNA8.
use super::{NativeIntentError, NativeOmittedInsertIntent};
use crate::{InsertFieldValue, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_testkit::fixtures::access::tcna2;

#[cfg(feature = "ace16-create")]
mod recovery;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn requests() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Null),
    ])]
}

fn literal(before: &[u8], rows: Vec<Vec<Option<Value>>>) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire =
        tcna2::wider_presence_wire("Table1", before.len(), digest.finalize().into(), rows);
    wire[..8].copy_from_slice(b"TCNA0009");
    wire[12..14].copy_from_slice(&9_u16.to_le_bytes());
    wire
}

#[test]
fn opaque_presence_has_exact_original_wire_and_closed_older_decoders() {
    let before = [0x49; 4096];
    let wire = literal(&before, vec![vec![None, Some(Value::Null)]]);
    let intent = NativeOmittedInsertIntent::new_retained_opaque(
        "Table1",
        requests(),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
    let decoded =
        NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_retained_opaque());
    assert!(!decoded.is_retained());
    assert!(matches!(
        decoded.rows()[0].values()[0],
        InsertFieldValue::Omitted
    ));
    assert!(matches!(
        decoded.rows()[0].values()[1],
        InsertFieldValue::Supplied(Value::Null)
    ));
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    for decode in [
        NativeOmittedInsertIntent::decode,
        NativeOmittedInsertIntent::decode_current,
        NativeOmittedInsertIntent::decode_retained,
    ] {
        assert!(matches!(
            decode(&wire, &mut budget(), &|| false),
            Err(NativeIntentError::UnsupportedVersion)
        ));
    }
    for family in [2_u8, 6, 7, 8] {
        let mut older = wire.clone();
        older[7] = b'0' + family;
        older[12..14].copy_from_slice(&u16::from(family).to_le_bytes());
        assert!(matches!(
            NativeOmittedInsertIntent::decode_retained_opaque(&older, &mut budget(), &|| false),
            Err(NativeIntentError::UnsupportedVersion)
        ));
    }
    for offset in [8, 14, 16, 64] {
        let mut malformed = wire.clone();
        malformed[offset] ^= 1;
        let mut work = ResourceBudget::new(0, 100, 100_000);
        assert!(
            matches!(
                NativeOmittedInsertIntent::decode_retained_opaque(&malformed, &mut work, &|| false),
                Err(NativeIntentError::InvalidRecord)
            ),
            "offset {offset}"
        );
        assert_eq!(work.remaining(Resource::Bytes), 0);
    }
    assert!(matches!(
        NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| true),
        Err(NativeIntentError::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(matches!(
        NativeOmittedInsertIntent::decode_retained_opaque(
            &wire,
            &mut ResourceBudget::new(0, 100, 100_000),
            &|| false
        ),
        Err(NativeIntentError::Inspect(crate::InspectError::Limit(_)))
    ));
}

#[test]
fn opaque_presence_binding_and_carriers_retain_native_and_wire_widths() {
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let text = "x".repeat(8193);
    let wire = literal(&before, vec![vec![None, Some(Value::Text(text))]]);
    let decoded =
        NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    before[64 * 1024 * 1024] = 1;
    assert!(matches!(
        decoded.verify_before(&before, &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    let mut oversized = wire;
    oversized[16..24].copy_from_slice(&(2_u64 * 1024 * 1024 * 1024 + 4096).to_le_bytes());
    assert!(matches!(
        NativeOmittedInsertIntent::decode_retained_opaque(
            &oversized,
            &mut ResourceBudget::new(0, 100, 100_000),
            &|| false
        ),
        Err(NativeIntentError::InvalidRecord)
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn opaque_presence_data_does_not_grant_journal_only_native_recovery() {
    use crate::{EditError, NativeRecoveryQualificationError};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0; 4096];
    let wire = literal(&before, vec![vec![None, Some(Value::Null)]]);
    let intent =
        NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| false).unwrap();
    let mut storage = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut storage,
            &mut journal,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        intent
            .qualify_retained_recovery(checked, CatalogScope::new(1690), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(_)
        ))
    ));
    assert_eq!(storage.writes, 0);
    assert_eq!(storage.bytes.get_ref(), &before);
}
