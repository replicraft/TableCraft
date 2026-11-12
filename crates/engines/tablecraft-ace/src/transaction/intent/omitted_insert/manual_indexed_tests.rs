//! Exact TCNA11 original presence is data, never an index/default permission.
#[cfg(feature = "ace16-create")]
mod recovery;

use super::{NativeIntentError as Error, NativeOmittedInsertIntent as Intent};
use crate::{InsertFieldValue, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_testkit::fixtures::access::tcna2;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 1_000_000_000)
}
fn literal(before: &[u8], table: &str, rows: Vec<Vec<Option<Value>>>) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = tcna2::wider_presence_wire(table, before.len(), digest.finalize().into(), rows);
    wire[..8].copy_from_slice(b"TCNA0011");
    wire[12..14].copy_from_slice(&11_u16.to_le_bytes());
    assert_eq!(&wire[56..60], &(table.len() as u32).to_le_bytes());
    assert_eq!(&wire[68..68 + table.len()], table.as_bytes());
    wire
}
fn requests() -> Vec<OmittedRowInsert> {
    vec![
        OmittedRowInsert::new(vec![InsertFieldValue::Omitted]),
        OmittedRowInsert::new(vec![InsertFieldValue::Supplied(Value::Null)]),
        OmittedRowInsert::new(vec![InsertFieldValue::Supplied(Value::Int32(i32::MIN))]),
    ]
}
fn original_rows() -> Vec<Vec<Option<Value>>> {
    vec![
        vec![None],
        vec![Some(Value::Null)],
        vec![Some(Value::Int32(i32::MIN))],
    ]
}

#[test]
fn literal_manual_indexed_wire_preserves_original_presence_and_exact_family() {
    let before = [0x11; 4096];
    let rows = requests();
    let pointer = rows[0].values().as_ptr();
    let intent =
        Intent::new_retained_manual_indexed("Archive", rows, &before, &mut budget(), &|| false)
            .unwrap();
    assert_eq!(intent.rows()[0].values().as_ptr(), pointer);
    let wire = literal(&before, "Archive", original_rows());
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
    let decoded = Intent::decode_retained_manual_indexed(&wire, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_retained_manual_indexed());
    assert!(!decoded.is_retained_keyless());
    assert!(!decoded.is_retained());
    assert!(!decoded.is_retained_opaque());
    assert!(matches!(
        decoded.rows()[0].values(),
        [InsertFieldValue::Omitted]
    ));
    assert!(matches!(
        decoded.rows()[1].values(),
        [InsertFieldValue::Supplied(Value::Null)]
    ));
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    for decode in [
        Intent::decode,
        Intent::decode_current,
        Intent::decode_retained,
        Intent::decode_retained_opaque,
        Intent::decode_retained_keyless,
    ] {
        assert!(matches!(
            decode(&wire, &mut budget(), &|| false),
            Err(Error::UnsupportedVersion)
        ));
    }
    for version in 1_u16..=10 {
        let mut old = wire.to_vec();
        old[..8].copy_from_slice(format!("TCNA{version:04}").as_bytes());
        old[12..14].copy_from_slice(&version.to_le_bytes());
        assert!(matches!(
            Intent::decode_retained_manual_indexed(&old, &mut budget(), &|| false),
            Err(Error::UnsupportedVersion)
        ));
    }
    assert!(crate::NativeInsertIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(crate::NativeEofInsertIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(crate::NativeCompleteInsertIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(crate::NativeClockInsertIntent::decode(&wire, &mut budget(), &|| false).is_err());
    let empty = literal(&before, "Archive", Vec::new());
    let decoded = Intent::decode_retained_manual_indexed(&empty, &mut budget(), &|| false).unwrap();
    assert!(decoded.rows().is_empty());
}

#[test]
fn malformed_framing_flags_presence_and_source_binding_refuse_without_decoder_fallback() {
    let before = [0x11; 4096];
    let wire = literal(&before, "Archive", original_rows());
    for end in 0..wire.len() {
        assert!(
            Intent::decode_retained_manual_indexed(&wire[..end], &mut budget(), &|| false).is_err()
        );
    }
    for offset in [8, 14, 64, 84] {
        let mut wrong = wire.to_vec();
        wrong[offset] ^= 1;
        assert!(
            Intent::decode_retained_manual_indexed(&wrong, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    let mut profile = wire.to_vec();
    profile[12..14].copy_from_slice(&10_u16.to_le_bytes());
    assert!(matches!(
        Intent::decode_retained_manual_indexed(&profile, &mut budget(), &|| false),
        Err(Error::UnsupportedProfile)
    ));
    let mut stale = before;
    stale[3000] ^= 1;
    let decoded = Intent::decode_retained_manual_indexed(&wire, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        decoded.verify_before(&stale, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
    assert!(matches!(
        Intent::decode_retained_manual_indexed(&wire, &mut budget(), &|| true),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(
        Intent::decode_retained_manual_indexed(&wire, &mut ResourceBudget::new(0, 0, 0), &|| false)
            .is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_manual_indexed_presence_cannot_authorize_native_effects() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0x11; 4096];
    let mut after = before.to_vec();
    after[3000] ^= 1;
    let wire = literal(&before, "Archive", original_rows());
    let mut target = MemoryStorage::new(before.to_vec());
    let mut file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut file,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = file.writes;
    let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
    let refused = Intent::decode_retained_manual_indexed(&wire, &mut budget(), &|| false)
        .unwrap()
        .qualify_retained_recovery(checked, CatalogScope::new(41100), &mut budget(), &|| false);
    assert!(matches!(
        refused,
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!((target.writes, file.writes), (0, writes));
    assert_eq!(target.bytes.get_ref(), &before);
}
