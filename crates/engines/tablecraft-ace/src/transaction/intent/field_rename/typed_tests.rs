//! Typed retained originals remain closed data until fresh native qualification.
use super::super::schema_field_test_wire::{self as wire, Facts, budget};
use super::{FieldRenameIntentError as Error, NativeFieldRenameIntent as Intent};
use tablecraft_core::limits::ResourceBudget;

#[cfg(feature = "ace16-create")]
mod recovery;

fn literal(before: &[u8], facts: Facts, table: &str, old: &str, new: &str) -> Vec<u8> {
    let mut bytes = wire::rename(before, facts, old, new);
    bytes[..8].copy_from_slice(b"TCFR0007");
    bytes[12..14].copy_from_slice(&7_u16.to_le_bytes());
    bytes.splice(80..86, table.bytes());
    let length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
    bytes[70..72].copy_from_slice(&(table.len() as u16).to_le_bytes());
    bytes
}

#[test]
fn typed_wire_roundtrips_exact_seven_without_reinterpreting_older_rename_data() {
    let before = vec![0; 8 * 4096];
    let facts = Facts {
        definition: 7,
        number: 2,
        column_id: 2,
    };
    for name in ["ExpandedTypedValue", "X", "token", "Token"] {
        let good = literal(&before, facts, "ExactValues", "Token", name);
        let decoded = Intent::decode_retained_typed(&good, &mut budget(), &|| false).unwrap();
        assert!(decoded.is_retained_typed());
        assert!(!decoded.is_retained_schema());
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), good);
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            Intent::decode_retained_schema(&good, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
        #[cfg(feature = "ace16-create")]
        assert!(matches!(
            Intent::decode(&good, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
        for profile in 1..=6 {
            let mut old = good.clone();
            old[..8].copy_from_slice(if profile == 6 {
                b"TCFR0006"
            } else {
                b"TCFR0001"
            });
            old[12..14].copy_from_slice(&u16::try_from(profile).unwrap().to_le_bytes());
            assert!(matches!(
                Intent::decode_retained_typed(&old, &mut budget(), &|| false),
                Err(Error::InvalidRecord)
            ));
        }
    }
    let zero = literal(
        &before,
        Facts {
            definition: 7,
            number: 0,
            column_id: 0,
        },
        "Keyless",
        "Token",
        "Renamed",
    );
    let decoded = Intent::decode_retained_typed(&zero, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.column_number(), 0);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), zero);
    let mut older = zero;
    older[..8].copy_from_slice(b"TCFR0006");
    older[12..14].copy_from_slice(&6_u16.to_le_bytes());
    assert!(matches!(
        Intent::decode_retained_schema(&older, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
}

#[test]
fn typed_framing_preflight_binding_and_caller_budgets_remain_exact() {
    let before = vec![0; 8 * 4096];
    let facts = Facts {
        definition: 7,
        number: 2,
        column_id: 2,
    };
    let good = literal(&before, facts, "ExactValues", "Token", "ExpandedTypedValue");
    for at in [0, 8, 12, 14, 68, 76] {
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
    let mut large = vec![0; 64 * 1024 * 1024 + 4096];
    let wire = literal(&large, facts, "ExactValues", "Token", "ExpandedTypedValue");
    let decoded = Intent::decode_retained_typed(&wire, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&large, &mut budget(), &|| false)
        .unwrap();
    large[64 * 1024 * 1024] ^= 1;
    assert!(matches!(
        decoded.verify_before(&large, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_typed_request_has_no_native_source_or_recovery_authority() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 8 * 4096];
    let facts = Facts {
        definition: 7,
        number: 2,
        column_id: 2,
    };
    let good = literal(&before, facts, "ExactValues", "Token", "ExpandedTypedValue");
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
            .qualify_retained_recovery(checked, CatalogScope::new(1700), &mut budget(), &|| false),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
}
