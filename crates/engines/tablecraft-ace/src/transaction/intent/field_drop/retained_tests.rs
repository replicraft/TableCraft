//! Independent retained request data stays separate from native save authority.
use super::super::schema_field_test_wire::{self as wire, Facts, budget};
use super::{FieldDropIntentError as Error, NativeFieldDropIntent as Intent};
use tablecraft_core::limits::ResourceBudget;

#[cfg(feature = "ace16-create")]
mod recovery;

#[test]
fn retained_schema_wire_is_closed_and_checks_borrowed_fields_before_allocation() {
    let before = vec![0; 8 * 4096];
    let facts = Facts {
        definition: 7,
        number: 1,
        column_id: 1,
    };
    let good = wire::drop(&before, facts, "NextText");
    let decoded = Intent::decode_retained(&good, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_retained());
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), good);
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    #[cfg(feature = "ace16-create")]
    assert!(matches!(
        Intent::decode(&good, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
    for at in [0, 8, 12, 14, 16, 64, 72] {
        let mut invalid = good.clone();
        invalid[at] ^= 1;
        assert!(matches!(
            Intent::decode_retained(&invalid, &mut ResourceBudget::new(0, 100, 100_000), &|| {
                false
            }),
            Err(Error::InvalidRecord | Error::Binding(_))
        ));
    }
    for (start, value) in [(56, 0_u32), (60, u32::MAX)] {
        let mut invalid = good.clone();
        invalid[start..start + 4].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(
            Intent::decode_retained(&invalid, &mut ResourceBudget::new(0, 100, 100_000), &|| {
                false
            }),
            Err(Error::InvalidRecord)
        ));
    }
    let mut old_magic = good.clone();
    old_magic[..8].copy_from_slice(b"TCFD0001");
    assert!(matches!(
        Intent::decode_retained(&old_magic, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
    #[cfg(feature = "ace16-create")]
    assert!(matches!(
        Intent::decode(&old_magic, &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
    assert!(matches!(
        Intent::decode_retained(&good, &mut budget(), &|| true),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(matches!(
        Intent::decode_retained(&good, &mut ResourceBudget::new(0, 100, 100_000), &|| false),
        Err(Error::Inspect(crate::InspectError::Limit(_)))
    ));
}

#[test]
fn retained_schema_binding_uses_native_geometry_without_legacy_image_quota() {
    // Data-only framing and hashes do not qualify these inert bytes as a database.
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let facts = Facts {
        definition: 7,
        number: 1,
        column_id: 1,
    };
    let good = wire::drop(&before, facts, "NextText");
    let decoded = Intent::decode_retained(&good, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    before[64 * 1024 * 1024] ^= 1;
    assert!(matches!(
        decoded.verify_before(&before, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
    let mut beyond_native = good;
    beyond_native[16..24].copy_from_slice(&(2_u64 * 1024 * 1024 * 1024 + 4096).to_le_bytes());
    assert!(matches!(
        Intent::decode_retained(
            &beyond_native,
            &mut ResourceBudget::new(0, 100, 100_000),
            &|| false
        ),
        Err(Error::Binding(_))
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_retained_request_cannot_acquire_native_recovery_authority() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0; 8 * 4096];
    let facts = Facts {
        definition: 7,
        number: 1,
        column_id: 1,
    };
    let encoded = wire::drop(&before, facts, "NextText");
    let decoded = Intent::decode_retained(&encoded, &mut budget(), &|| false).unwrap();
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
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
        decoded
            .qualify_retained_recovery(checked, CatalogScope::new(1600), &mut budget(), &|| false),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
