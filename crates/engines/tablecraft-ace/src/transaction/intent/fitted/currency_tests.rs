//! TCNI19 retains scaled Currency witnesses, separate from native admission.
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};

#[cfg(feature = "ace16-create")]
mod recovery;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 2_000_000_000)
}

fn groups(row: usize, expected: Option<i64>, value: Option<i64>) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained(
            "Money",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                row,
                expected.map_or(Value::Null, |value| {
                    Value::Currency(Currency::from_scaled(value))
                }),
                value.map_or(Value::Null, |value| {
                    Value::Currency(Currency::from_scaled(value))
                }),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(before: &[u8], row: u64, expected: Option<i64>, value: Option<i64>) -> Vec<u8> {
    literal_column(before, row, "Table1", "Money", expected, value)
}

fn literal_column(
    before: &[u8],
    row: u64,
    table: &str,
    column: &str,
    expected: Option<i64>,
    value: Option<i64>,
) -> Vec<u8> {
    let payload = 8 * (usize::from(expected.is_some()) + usize::from(value.is_some()));
    let mut wire = b"TCNI0019".to_vec();
    wire.extend(
        (120_u32 + table.len() as u32 + column.len() as u32 + payload as u32).to_le_bytes(),
    );
    wire.extend([19, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    wire.extend(digest.finalize());
    for number in [table.len() as u32, 1, 1, 0] {
        wire.extend(number.to_le_bytes());
    }
    wire.extend(table.as_bytes());
    wire.extend((48_u32 + column.len() as u32 + payload as u32).to_le_bytes());
    wire.extend((column.len() as u32).to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([0; 8]);
    wire.extend(column.as_bytes());
    wire.extend((28_u32 + payload as u32).to_le_bytes());
    wire.extend(row.to_le_bytes());
    for cell in [expected, value] {
        wire.extend([if cell.is_some() { 4 } else { 0 }, 0, 0, 0]);
        wire.extend((if cell.is_some() { 8_u32 } else { 0_u32 }).to_le_bytes());
        if let Some(value) = cell {
            wire.extend(value.to_le_bytes());
        }
    }
    wire
}

#[test]
fn currency_original_wire_is_literal_closed_and_preserves_native_row_ordinals() {
    let before = [0x73; 4096];
    for (expected, value) in [
        (None, Some(-20_001)),
        (Some(12_345), None),
        (Some(i64::MIN), Some(i64::MAX)),
    ] {
        let wire = literal(&before, 70_000, expected, value);
        let original = NativeFittedEditIntent::new_retained(
            "Table1",
            groups(70_000, expected, value),
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            original
                .encode_retained_currency(&mut budget(), &|| false)
                .unwrap(),
            wire
        );
        assert!(NativeFittedEditIntent::decode_retained(&wire, &mut budget(), &|| false).is_err());
        assert!(
            NativeFittedEditIntent::decode_retained_opaque(&wire, &mut budget(), &|| false)
                .is_err()
        );
        assert!(NativeFittedEditIntent::decode(&wire, &mut budget(), &|| false).is_err());
        let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.profile(),
            NativeUpdateProfile::Ace16RetainedCurrencyRows
        );
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        for at in [0, 8, 14, 16, 68] {
            let mut invalid = wire.clone();
            invalid[at] ^= 1;
            let mut work = ResourceBudget::new(0, 100, 100_000);
            let error =
                NativeFittedEditIntent::decode_retained_currency(&invalid, &mut work, &|| false)
                    .err()
                    .expect("invalid header must refuse before allocation");
            assert!(
                matches!(error, NativeIntentError::InvalidRecord),
                "offset {at}: {error}"
            );
            assert_eq!(work.remaining(tablecraft_core::limits::Resource::Bytes), 0);
        }
        let mut wrong_profile = wire.clone();
        wrong_profile[12..14].copy_from_slice(&17_u16.to_le_bytes());
        assert!(matches!(
            NativeFittedEditIntent::decode_retained_currency(
                &wrong_profile,
                &mut ResourceBudget::new(0, 100, 100_000),
                &|| false,
            ),
            Err(NativeIntentError::UnsupportedProfile)
        ));
        assert!(matches!(
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| true),
            Err(NativeIntentError::Inspect(crate::InspectError::Cancelled))
        ));
        assert!(matches!(
            NativeUpdateIntent::decode(&wire, &mut ResourceBudget::new(0, 100, 100_000), &|| false),
            Err(NativeIntentError::Inspect(crate::InspectError::Limit(_)))
        ));
    }
}

#[test]
fn currency_data_binding_uses_native_file_width_without_old_image_quota() {
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let mut wire = literal(&before, 0, None, Some(42));
    let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    before[64 * 1024 * 1024] = 1;
    assert!(matches!(
        decoded.verify_before(&before, &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    wire[16..24].copy_from_slice(&(2_u64 * 1024 * 1024 * 1024 + 4096).to_le_bytes());
    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut ResourceBudget::new(0, 100, 100_000), &|| false),
        Err(NativeIntentError::InvalidRecord)
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn currency_request_data_does_not_enable_native_mutation_in_a_journal_only_build() {
    use crate::{EditError, NativeRecoveryQualificationError};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0; 4096];
    let wire = literal(&before, 0, None, Some(42));
    let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
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
        decoded
            .qualify_retained_recovery(checked, CatalogScope::new(1620), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(_)
        ))
    ));
    assert_eq!(storage.writes, 0);
    assert_eq!(storage.bytes.get_ref(), &before);
}
