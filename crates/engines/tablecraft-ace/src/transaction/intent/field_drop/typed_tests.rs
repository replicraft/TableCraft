//! Closed typed tail originals are data, never native cleanup authority.
use super::super::schema_field_test_wire::{self as wire, Facts, budget};
use super::{FieldDropIntentError as Error, NativeFieldDropIntent as Intent};
use tablecraft_core::limits::ResourceBudget;

#[cfg(feature = "ace16-create")]
mod recovery;

fn literal(before: &[u8], facts: Facts, table: &str, column: &str) -> Vec<u8> {
    let mut bytes = wire::drop(before, facts, column);
    bytes[..8].copy_from_slice(b"TCFD0004");
    bytes[12..14].copy_from_slice(&4_u16.to_le_bytes());
    bytes.splice(76..82, table.bytes());
    let length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
    bytes[68..70].copy_from_slice(&(table.len() as u16).to_le_bytes());
    bytes
}

#[test]
fn typed_drop_data_roundtrips_independent_identity_and_keeps_one_through_three_closed() {
    let before = vec![0; 8 * 4096];
    let good = literal(
        &before,
        Facts {
            definition: 7,
            number: 3,
            column_id: 3,
        },
        "ExactValues",
        "Amount",
    );
    let decoded = Intent::decode_retained_typed(&good, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_retained_typed());
    assert!(!decoded.is_retained());
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), good);
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        Intent::decode_retained(&good, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
    #[cfg(feature = "ace16-create")]
    for decode in [Intent::decode, Intent::decode_current_tail] {
        assert!(matches!(
            decode(&good, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
    }
    for profile in 1_u16..=3 {
        let mut old = good.clone();
        old[7] = b'0' + profile as u8;
        old[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(matches!(
            Intent::decode_retained_typed(&old, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
    }
    let zero = literal(
        &before,
        Facts {
            definition: 7,
            number: 0,
            column_id: 0,
        },
        "Keyless",
        "Amount",
    );
    let decoded = Intent::decode_retained_typed(&zero, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.column_number(), 0);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), zero);
    let mut older = zero;
    older[..8].copy_from_slice(b"TCFD0003");
    older[12..14].copy_from_slice(&3_u16.to_le_bytes());
    assert!(matches!(
        Intent::decode_retained(&older, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));

    for at in [0, 8, 12, 14, 72] {
        let mut invalid = good.clone();
        invalid[at] ^= 1;
        assert!(
            matches!(
                Intent::decode_retained_typed(&invalid, &mut budget(), &|| false),
                Err(Error::InvalidRecord)
            ),
            "offset {at}"
        );
    }
    let mut invalid = good.clone();
    invalid[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Intent::decode_retained_typed(&invalid, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
    assert!(matches!(
        Intent::decode_retained_typed(&good, &mut budget(), &|| true),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(matches!(
        Intent::decode_retained_typed(&good, &mut ResourceBudget::new(0, 100, 100_000), &|| false),
        Err(Error::Inspect(crate::InspectError::Limit(_)))
    ));
}

#[test]
fn typed_drop_original_binding_covers_actual_native_geometry() {
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let good = literal(
        &before,
        Facts {
            definition: 7,
            number: 3,
            column_id: 3,
        },
        "ExactValues",
        "Amount",
    );
    let decoded = Intent::decode_retained_typed(&good, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    before[64 * 1024 * 1024] ^= 1;
    assert!(matches!(
        decoded.verify_before(&before, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
    let mut invalid = good;
    invalid[16..24].copy_from_slice(&(2_u64 * 1024 * 1024 * 1024 + 4096).to_le_bytes());
    assert!(matches!(
        Intent::decode_retained_typed(&invalid, &mut budget(), &|| false),
        Err(Error::Binding(_))
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_typed_drop_cannot_acquire_native_recovery_authority() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 8 * 4096];
    let good = literal(
        &before,
        Facts {
            definition: 7,
            number: 3,
            column_id: 3,
        },
        "ExactValues",
        "Amount",
    );
    let mut target = MemoryStorage::new(before.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        Intent::decode_retained_typed(&good, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1800), &mut budget(), &|| false),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
}
