//! TCFD5 carries one original non-tail request, never decoded mutation authority.
use super::super::schema_field_test_wire::{self as wire, Facts, budget};
use super::{FieldDropIntentError as Error, NativeFieldDropIntent as Intent};
use tablecraft_core::limits::ResourceBudget;

#[cfg(feature = "ace16-create")]
mod recovery;

fn literal(before: &[u8], facts: Facts, column: &str) -> Vec<u8> {
    let mut bytes = wire::drop(before, facts, column);
    bytes[..8].copy_from_slice(b"TCFD0005");
    bytes[12..14].copy_from_slice(&5_u16.to_le_bytes());
    bytes
}

#[test]
fn non_tail_original_roundtrips_literal_identity_and_rejects_every_tail_language() {
    let before = vec![0; 12 * 4096];
    for number in [0, 1] {
        let good = literal(
            &before,
            Facts {
                definition: 11,
                number,
                column_id: 7,
            },
            "Score",
        );
        let decoded = Intent::decode_retained_non_tail(&good, &mut budget(), &|| false).unwrap();
        assert!(decoded.is_retained_non_tail());
        assert!(!decoded.is_retained());
        assert!(!decoded.is_retained_typed());
        assert_eq!(decoded.column_number(), usize::from(number));
        assert_eq!(decoded.column_id(), 7);
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), good);
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        for decode in [Intent::decode_retained, Intent::decode_retained_typed] {
            assert!(matches!(
                decode(&good, &mut budget(), &|| false),
                Err(Error::InvalidRecord)
            ));
        }
        #[cfg(feature = "ace16-create")]
        for decode in [Intent::decode, Intent::decode_current_tail] {
            assert!(matches!(
                decode(&good, &mut budget(), &|| false),
                Err(Error::InvalidRecord)
            ));
        }
        for profile in 1_u16..=4 {
            let mut old = good.clone();
            old[7] = b'0' + profile as u8;
            old[12..14].copy_from_slice(&profile.to_le_bytes());
            assert!(
                matches!(
                    Intent::decode_retained_non_tail(&old, &mut budget(), &|| false),
                    Err(Error::InvalidRecord)
                ),
                "profile {profile}"
            );
        }
        for at in [0, 8, 12, 14, 72] {
            let mut invalid = good.clone();
            invalid[at] ^= 1;
            assert!(
                matches!(
                    Intent::decode_retained_non_tail(&invalid, &mut budget(), &|| false),
                    Err(Error::InvalidRecord)
                ),
                "offset {at}"
            );
        }
        let mut outside = good.clone();
        outside[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Intent::decode_retained_non_tail(&outside, &mut budget(), &|| false),
            Err(Error::InvalidRecord)
        ));
        assert!(matches!(
            Intent::decode_retained_non_tail(&good, &mut budget(), &|| true),
            Err(Error::Inspect(crate::InspectError::Cancelled))
        ));
        assert!(matches!(
            Intent::decode_retained_non_tail(
                &good,
                &mut ResourceBudget::new(0, 100, 100_000),
                &|| false
            ),
            Err(Error::Inspect(crate::InspectError::Limit(_)))
        ));
    }
}

#[test]
fn non_tail_binding_covers_the_whole_native_image_beyond_legacy_capture_length() {
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let good = literal(
        &before,
        Facts {
            definition: 11,
            number: 1,
            column_id: 1,
        },
        "Score",
    );
    let decoded = Intent::decode_retained_non_tail(&good, &mut budget(), &|| false).unwrap();
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
        Intent::decode_retained_non_tail(&invalid, &mut budget(), &|| false),
        Err(Error::Binding(_))
    ));
}

#[test]
fn non_tail_names_follow_native_utf16_units_without_widening_legacy_ascii() {
    let before = vec![0; 12 * 4096];
    let facts = Facts {
        definition: 11,
        number: 1,
        column_id: 1,
    };
    for (table, column, valid) in [
        ("界".repeat(64), "界".repeat(64), true),
        ("🦀".repeat(32), "Score".into(), true),
        ("界".repeat(65), "Score".into(), false),
        ("Table1".into(), "🦀".repeat(33), false),
    ] {
        let mut bytes = literal(&before, facts, &column);
        bytes.splice(76..82, table.bytes());
        let length = bytes.len() as u32;
        bytes[8..12].copy_from_slice(&length.to_le_bytes());
        bytes[68..70].copy_from_slice(&(table.len() as u16).to_le_bytes());
        let decoded = Intent::decode_retained_non_tail(&bytes, &mut budget(), &|| false);
        if valid {
            let decoded = decoded.unwrap();
            assert_eq!(decoded.table(), table);
            assert_eq!(decoded.column(), column);
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), bytes);
            assert!(bytes.len() <= Intent::MAX_NON_TAIL_ENCODED_BYTES);
            bytes[..8].copy_from_slice(b"TCFD0004");
            bytes[12..14].copy_from_slice(&4_u16.to_le_bytes());
            assert!(matches!(
                Intent::decode_retained_typed(&bytes, &mut budget(), &|| false),
                Err(Error::InvalidRecord)
            ));
        } else {
            assert!(matches!(decoded, Err(Error::InvalidRecord)));
        }
    }
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_non_tail_data_cannot_qualify_native_v3_recovery() {
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0; 12 * 4096];
    let good = literal(
        &before,
        Facts {
            definition: 11,
            number: 1,
            column_id: 1,
        },
        "Score",
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
        Intent::decode_retained_non_tail(&good, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(4500), &mut budget(), &|| false),
        Err(Error::UnsupportedSource)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
