//! TCNI23 preserves original bits as data without granting stored-bit authority.
use super::boolean_tests::{budget, groups};
use crate::{NativeFittedEditIntent, NativeIntentError, NativeUpdateIntent, NativeUpdateProfile};
use tablecraft_core::values::Value;

#[cfg(feature = "ace16-create")]
mod recovery;

fn literal(before: &[u8], row: u64, expected: bool, value: bool) -> Vec<u8> {
    let mut wire = super::boolean_tests::literal(before, "Table1", "Enabled", row, expected, value);
    wire[..8].copy_from_slice(b"TCNI0023");
    wire[12..14].copy_from_slice(&23_u16.to_le_bytes());
    wire
}

#[test]
fn missing_boolean_originals_keep_exact_data_and_old_families_closed() {
    let before = [0x23; 4096];
    for value in [false, true] {
        let wire = literal(&before, 70_000, false, value);
        let original = NativeFittedEditIntent::new_retained(
            "Table1",
            groups("Enabled", 70_000, false, Value::Boolean(value)),
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            original
                .encode_retained_boolean_materialization(&mut budget(), &|| false)
                .unwrap(),
            wire
        );
        let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.profile(),
            NativeUpdateProfile::Ace16RetainedBooleanMaterialization
        );
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        for decode in [
            NativeFittedEditIntent::decode,
            NativeFittedEditIntent::decode_retained,
            NativeFittedEditIntent::decode_retained_opaque,
            NativeFittedEditIntent::decode_retained_currency,
            NativeFittedEditIntent::decode_retained_boolean,
            NativeFittedEditIntent::decode_retained_fixed_scalars,
            NativeFittedEditIntent::decode_retained_wide_values,
        ] {
            assert!(decode(&wire, &mut budget(), &|| false).is_err());
        }
        let mut invalid = wire.clone();
        *invalid.last_mut().unwrap() = 2;
        assert!(matches!(
            NativeFittedEditIntent::decode_retained_boolean_materialization(
                &invalid,
                &mut budget(),
                &|| false
            ),
            Err(NativeIntentError::InvalidRecord)
        ));
        for offset in [0, 8, 14, 68] {
            let mut invalid = wire.clone();
            invalid[offset] ^= 1;
            assert!(
                matches!(
                    NativeFittedEditIntent::decode_retained_boolean_materialization(
                        &invalid,
                        &mut budget(),
                        &|| false
                    ),
                    Err(NativeIntentError::InvalidRecord)
                ),
                "offset {offset}"
            );
        }
    }
    let mut wrong_profile = literal(&before, 0, false, true);
    wrong_profile[12..14].copy_from_slice(&22_u16.to_le_bytes());
    assert!(matches!(
        NativeFittedEditIntent::decode_retained_boolean_materialization(
            &wrong_profile,
            &mut budget(),
            &|| false
        ),
        Err(NativeIntentError::UnsupportedProfile)
    ));
    let old = super::boolean_tests::literal(&before, "Table1", "Enabled", 0, false, true);
    assert!(matches!(
        NativeFittedEditIntent::decode_retained_boolean_materialization(
            &old,
            &mut budget(),
            &|| false
        ),
        Err(NativeIntentError::UnsupportedVersion)
    ));
    let good = literal(&before, 0, false, true);
    assert!(matches!(
        NativeFittedEditIntent::decode_retained_boolean_materialization(
            &good,
            &mut budget(),
            &|| true
        ),
        Err(NativeIntentError::Inspect(crate::InspectError::Cancelled))
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_data_does_not_grant_missing_bit_native_recovery() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 4096];
    let wire = literal(&before, 0, false, true);
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
        NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(2300), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!(target.writes, 0);
}
