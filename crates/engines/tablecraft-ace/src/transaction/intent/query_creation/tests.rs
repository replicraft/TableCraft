//! Literal TCQC data and complete binding, independent of the production encoder.
#[cfg(feature = "ace16-create")]
mod recovery;

use super::{NativeQueryCreationIntent as Intent, QueryCreationIntentError as Error};
use crate::{InspectError, NativeQueryExpressionEditsIntent, NativeQueryExpressionIntent};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::LegacyDateTime,
};

#[cfg(not(feature = "ace16-create"))]
use tablecraft_io::journal;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_model::CatalogScope;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_testkit::{
    fixtures::access::saved_queries::creation as raw, support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn literal(
    before: &[u8],
    names: [&str; 2],
    native_id: i32,
    value: i32,
    date: LegacyDateTime,
) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = b"TCQC0001".to_vec();
    wire.extend(((80 + names[0].len() + names[1].len()) as u32).to_le_bytes());
    wire.extend(1_u16.to_le_bytes());
    wire.extend(0_u16.to_le_bytes());
    wire.extend((before.len() as u64).to_le_bytes());
    wire.extend(digest.finalize());
    wire.extend(native_id.to_le_bytes());
    wire.extend(value.to_le_bytes());
    wire.extend(date.to_bits().to_le_bytes());
    wire.extend((names[0].len() as u16).to_le_bytes());
    wire.extend((names[1].len() as u16).to_le_bytes());
    wire.extend(0_u32.to_le_bytes());
    wire.extend(names[0].as_bytes());
    wire.extend(names[1].as_bytes());
    wire
}

#[test]
fn literal_creation_data_retains_signed_constants_dates_unicode_and_full_source_digest() {
    let before = vec![0; 9 * 4096];
    for (native_id, value, days) in [
        (i32::MIN + 1, i32::MIN, -2.5),
        (-17, 0, 0.0),
        (-1, i32::MAX, 45000.5),
    ] {
        let date = LegacyDateTime::from_ole_days(days).unwrap();
        let bytes = literal(
            &before,
            ["Résumé 新", "\u{feff}Heading"],
            native_id,
            value,
            date,
        );
        let original = Intent::decode(&bytes, &mut budget(), &|| false).unwrap();
        assert_eq!(original.name(), "Résumé 新");
        assert_eq!(original.alias(), "\u{feff}Heading");
        assert_eq!(original.native_query_id(), native_id);
        assert_eq!(original.value(), value);
        assert_eq!(original.captured_at().to_bits(), date.to_bits());
        assert_eq!(original.before_length(), before.len() as u64);
        assert_eq!(original.encode(&mut budget(), &|| false).unwrap(), bytes);
        original
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        assert!(NativeQueryExpressionIntent::decode(&bytes, &mut budget(), &|| false).is_err());
        assert!(
            NativeQueryExpressionEditsIntent::decode(&bytes, &mut budget(), &|| false).is_err()
        );
    }
}

#[test]
fn malformed_versions_reserved_geometry_names_ids_and_date_refuse_before_allocation() {
    let before = vec![0; 9 * 4096];
    let date = LegacyDateTime::from_ole_days(45000.5).unwrap();
    let bytes = literal(&before, ["Created", "Answer"], -17, 43, date);
    for end in 0..bytes.len() {
        assert!(
            Intent::decode(&bytes[..end], &mut budget(), &|| false).is_err(),
            "truncation {end}"
        );
    }
    for offset in [0, 8, 12, 14, 15, 72, 74, 76, 77, 78, 79] {
        // NO_CLONE: Each malformed control owns its mutation while the literal remains unchanged for other controls.
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        assert!(
            Intent::decode(&bad, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    // NO_CLONE: Each malformed control owns its mutation while the literal remains unchanged for other controls.
    let mut version = bytes.clone();
    version[7] = b'2';
    assert!(matches!(
        Intent::decode(&version, &mut budget(), &|| false),
        Err(Error::UnsupportedVersion)
    ));
    for old in [b"TCQE0001", b"TCQE0002"] {
        // NO_CLONE: Each malformed control owns its mutation while the literal remains unchanged for other controls.
        let mut wrong_family = bytes.clone();
        wrong_family[..8].copy_from_slice(old);
        assert!(matches!(
            Intent::decode(&wrong_family, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
    }
    for id in [i32::MIN, 0, i32::MAX] {
        // NO_CLONE: Each malformed control owns its mutation while the literal remains unchanged for other controls.
        let mut bad = bytes.clone();
        bad[56..60].copy_from_slice(&id.to_le_bytes());
        assert!(Intent::decode(&bad, &mut budget(), &|| false).is_err());
    }
    for length in [0_u64, 1, u64::MAX] {
        // NO_CLONE: Each malformed control owns its mutation while the literal remains unchanged for other controls.
        let mut bad = bytes.clone();
        bad[16..24].copy_from_slice(&length.to_le_bytes());
        assert!(Intent::decode(&bad, &mut budget(), &|| false).is_err());
    }
    for invalid in [f64::NAN.to_bits(), f64::INFINITY.to_bits()] {
        // NO_CLONE: Each malformed control owns its mutation while the literal remains unchanged for other controls.
        let mut bad = bytes.clone();
        bad[64..72].copy_from_slice(&invalid.to_le_bytes());
        assert!(Intent::decode(&bad, &mut budget(), &|| false).is_err());
    }
    for byte in [0_u8, 0xff] {
        // NO_CLONE: Each malformed control owns its mutation while the literal remains unchanged for other controls.
        let mut bad = bytes.clone();
        bad[80] = byte;
        assert!(Intent::decode(&bad, &mut budget(), &|| false).is_err());
    }
    let invalid_name = literal(&before, [&"x".repeat(65), "Answer"], -17, 43, date);
    assert!(Intent::decode(&invalid_name, &mut budget(), &|| false).is_err());
    let mut unpaid = ResourceBudget::new(0, 1_000_000, 500_000_000);
    assert!(
        matches!(Intent::decode(&bytes, &mut unpaid, &|| false), Err(Error::Inspect(InspectError::Limit(error))) if error.resource == Resource::Bytes)
    );
    assert!(matches!(
        Intent::decode(&bytes, &mut budget(), &|| true),
        Err(Error::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn whole_before_binding_includes_protected_nonquery_bytes() {
    let before = vec![0; 9 * 4096];
    let bytes = literal(
        &before,
        ["Created", "Answer"],
        -17,
        43,
        LegacyDateTime::from_ole_days(0.0).unwrap(),
    );
    let original = Intent::decode(&bytes, &mut budget(), &|| false).unwrap();
    let mut other = before;
    other[3 * 4096 + 3000] ^= 1;
    assert!(matches!(
        original.verify_before(&other, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_decodes_real_creation_data_but_refuses_native_qualification_without_effects() {
    let date = LegacyDateTime::from_ole_days(45000.5).unwrap();
    let case = raw::source("Created", "Answer", 43, date);
    let wire = literal(
        &case.before,
        ["Created", "Answer"],
        case.native_query_id,
        43,
        date,
    );
    let original = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
    original
        .verify_before(&case.before, &mut budget(), &|| false)
        .unwrap();
    let mut target = MemoryStorage::new(case.before);
    let mut file = MemoryStorage::new(Vec::new());
    let before = target.bytes.get_ref().as_slice();
    // NO_CLONE: The journal setup borrows an independent immutable expected
    // BEFORE while MemoryStorage remains a mutable protocol target.
    let expected_before = before.to_vec();
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut file,
            &expected_before,
            &case.expected,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = file.writes;
    let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        original.qualify_retained_recovery(
            checked,
            CatalogScope::new(31901),
            &mut budget(),
            &|| false
        ),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(file.writes, writes);
    assert_eq!(target.bytes.get_ref(), &expected_before);
}
