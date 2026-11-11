//! TCNA10 is original presence data, distinct from every generated/legacy family.
use super::{NativeIntentError as Error, NativeOmittedInsertIntent as Intent};
use crate::{InsertFieldValue, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_testkit::fixtures::access::tcna2;

#[cfg(feature = "ace16-create")]
mod recovery;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 1_000_000_000)
}

fn literal(before: &[u8], rows: Vec<Vec<Option<Value>>>) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut bytes =
        tcna2::wider_presence_wire("Keyless", before.len(), digest.finalize().into(), rows);
    bytes[..8].copy_from_slice(b"TCNA0010");
    bytes[12..14].copy_from_slice(&10_u16.to_le_bytes());
    bytes
}

#[test]
fn keyless_literal_wire_keeps_omitted_null_value_order_and_all_old_decoders_closed() {
    let before = vec![0; 4096];
    let rows = vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Null),
        InsertFieldValue::Supplied(Value::Int32(77)),
    ])];
    let good = literal(
        &before,
        vec![vec![None, Some(Value::Null), Some(Value::Int32(77))]],
    );
    let intent =
        Intent::new_retained_keyless("Keyless", rows, &before, &mut budget(), &|| false).unwrap();
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), good);
    let decoded = Intent::decode_retained_keyless(&good, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_retained_keyless());
    assert!(!decoded.is_retained());
    assert!(!decoded.is_retained_opaque());
    assert!(matches!(
        decoded.rows()[0].values(),
        [
            InsertFieldValue::Omitted,
            InsertFieldValue::Supplied(Value::Null),
            InsertFieldValue::Supplied(Value::Int32(77))
        ]
    ));
    for decode in [
        Intent::decode,
        Intent::decode_current,
        Intent::decode_retained,
        Intent::decode_retained_opaque,
    ] {
        assert!(matches!(
            decode(&good, &mut budget(), &|| false),
            Err(Error::UnsupportedVersion)
        ));
    }
    for family in 1_u16..=9 {
        let mut old = good.clone();
        old[..8].copy_from_slice(format!("TCNA{family:04}").as_bytes());
        old[12..14].copy_from_slice(&family.to_le_bytes());
        assert!(
            matches!(
                Intent::decode_retained_keyless(&old, &mut budget(), &|| false),
                Err(Error::UnsupportedVersion)
            ),
            "old family {family}"
        );
    }
    assert!(crate::NativeInsertIntent::decode(&good, &mut budget(), &|| false).is_err());
    assert!(crate::NativeEofInsertIntent::decode(&good, &mut budget(), &|| false).is_err());
    assert!(crate::NativeCompleteInsertIntent::decode(&good, &mut budget(), &|| false).is_err());
    assert!(crate::NativeClockInsertIntent::decode(&good, &mut budget(), &|| false).is_err());
    for offset in [8, 14, 64] {
        let mut malformed = good.clone();
        malformed[offset] ^= 1;
        assert!(
            matches!(
                Intent::decode_retained_keyless(&malformed, &mut budget(), &|| false),
                Err(Error::InvalidRecord)
            ),
            "offset {offset}"
        );
    }
    let mut profile = good.clone();
    profile[12..14].copy_from_slice(&8_u16.to_le_bytes());
    assert!(matches!(
        Intent::decode_retained_keyless(&profile, &mut budget(), &|| false),
        Err(Error::UnsupportedProfile)
    ));
    assert!(matches!(
        Intent::decode_retained_keyless(&good, &mut budget(), &|| true),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
}

#[test]
fn keyless_original_uses_native_binding_and_wire_counts_without_legacy_quotas() {
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let good = literal(&before, Vec::new());
    let intent = Intent::decode_retained_keyless(&good, &mut budget(), &|| false).unwrap();
    intent
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    before[64 * 1024 * 1024] ^= 1;
    assert!(matches!(
        intent.verify_before(&before, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
    let rows = (0..16_385)
        .map(|_| OmittedRowInsert::new(vec![InsertFieldValue::Omitted]))
        .collect();
    let intent =
        Intent::new_retained_keyless("Keyless", rows, &[0; 4096], &mut budget(), &|| false)
            .unwrap();
    let encoded = intent.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(
        Intent::decode_retained_keyless(&encoded, &mut budget(), &|| false)
            .unwrap()
            .rows()
            .len(),
        16_385
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_keyless_data_cannot_acquire_native_recovery_authority() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 4096];
    let mut target = MemoryStorage::new(before.clone());
    let mut file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut file,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
    let intent =
        Intent::decode_retained_keyless(&literal(&before, Vec::new()), &mut budget(), &|| false)
            .unwrap();
    assert!(matches!(
        intent
            .qualify_retained_recovery(checked, CatalogScope::new(4620), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
