//! TCNI21 exact carriers and protocol framing do not authorize native mutation.
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, LegacyDateTime, Single, Value},
};

#[cfg(feature = "ace16-create")]
mod recovery;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 512 * 1024 * 1024)
}

fn groups(column: &str, expected: Value, value: Value) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained_fixed_scalars(
            column,
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(70_000, expected, value)]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(
    before: &[u8],
    column: &str,
    tag: u8,
    expected: &[u8],
    value_tag: u8,
    value: &[u8],
) -> Vec<u8> {
    let table = "Measurements";
    let cell = 28 + expected.len() + value.len();
    let group = 20 + column.len() + cell;
    let total = 72 + table.len() + group;
    let mut bytes = b"TCNI0021".to_vec();
    bytes.extend((total as u32).to_le_bytes());
    bytes.extend([21, 0, 0, 0]);
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    bytes.extend(hash.finalize());
    for value in [table.len() as u32, 1, 1, 0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(table.as_bytes());
    for value in [group as u32, column.len() as u32, 1, 0, 0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(column.as_bytes());
    bytes.extend((cell as u32).to_le_bytes());
    bytes.extend(70_000_u64.to_le_bytes());
    for (tag, payload) in [(tag, expected), (value_tag, value)] {
        bytes.extend([tag, 0, 0, 0]);
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
    }
    bytes
}

#[test]
fn six_exact_scalar_carriers_match_independent_wire_and_keep_older_grammars_closed() {
    let before = [0x56; 4096];
    for (column, tag, expected, replacement, initial, value) in [
        (
            "ByteValue",
            8,
            Value::Byte(17),
            Value::Byte(255),
            vec![17],
            vec![255],
        ),
        (
            "ShortValue",
            9,
            Value::Int16(-1234),
            Value::Int16(i16::MIN),
            (-1234_i16).to_le_bytes().to_vec(),
            i16::MIN.to_le_bytes().to_vec(),
        ),
        (
            "LargeValue",
            3,
            Value::Int64(9_007_199_254_740_993),
            Value::Int64(-9_007_199_254_740_993),
            9_007_199_254_740_993_i64.to_le_bytes().to_vec(),
            (-9_007_199_254_740_993_i64).to_le_bytes().to_vec(),
        ),
        (
            "SingleValue",
            10,
            Value::Single(Single::from_bits(0x3fa0_0000)),
            Value::Single(Single::from_bits(0xc020_0000)),
            0x3fa0_0000_u32.to_le_bytes().to_vec(),
            0xc020_0000_u32.to_le_bytes().to_vec(),
        ),
        (
            "DoubleValue",
            11,
            Value::Double(Double::from_bits(0x3ff3_c0ca_428c_59dd)),
            Value::Double(Double::from_bits(0xc006_0000_0000_0000)),
            0x3ff3_c0ca_428c_59dd_u64.to_le_bytes().to_vec(),
            0xc006_0000_0000_0000_u64.to_le_bytes().to_vec(),
        ),
        (
            "DateValue",
            5,
            Value::DateTime(LegacyDateTime::from_bits(0x4004_0000_0000_0000).unwrap()),
            Value::DateTime(LegacyDateTime::from_bits(0x400c_0000_0000_0000).unwrap()),
            0x4004_0000_0000_0000_u64.to_le_bytes().to_vec(),
            0x400c_0000_0000_0000_u64.to_le_bytes().to_vec(),
        ),
    ] {
        for (expected, replacement, expected_tag, expected_bytes, value_tag, value_bytes) in [
            (
                expected.clone(),
                replacement,
                tag,
                initial.clone(),
                tag,
                value,
            ),
            (expected, Value::Null, tag, initial, 0, Vec::new()),
            (Value::Null, Value::Null, 0, Vec::new(), 0, Vec::new()),
        ] {
            let wire = literal(
                &before,
                column,
                expected_tag,
                &expected_bytes,
                value_tag,
                &value_bytes,
            );
            let original = NativeFittedEditIntent::new_retained_fixed_scalars(
                "Measurements",
                groups(column, expected, replacement),
                &before,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                original
                    .encode_retained_fixed_scalars(&mut budget(), &|| false)
                    .unwrap(),
                wire
            );
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(
                decoded.profile(),
                NativeUpdateProfile::Ace16RetainedFixedScalarRows
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
            ] {
                assert!(decode(&wire, &mut budget(), &|| false).is_err());
            }
        }
    }
}

#[test]
fn fixed_scalar_preflight_refuses_nonfinite_payloads_wrong_widths_and_variable_groups() {
    let before = [0; 4096];
    let wire = literal(
        &before,
        "SingleValue",
        10,
        &0x3fa0_0000_u32.to_le_bytes(),
        10,
        &0xc020_0000_u32.to_le_bytes(),
    );
    let carrier = 72 + "Measurements".len() + 20 + "SingleValue".len() + 12;
    for (offset, value) in [
        (8, 0),
        (14, 1),
        (68, 1),
        (72 + "Measurements".len() + 12, 1),
        (carrier, 1),
        (carrier + 4, 8),
    ] {
        let mut malformed = wire.clone();
        malformed[offset] = value;
        assert!(
            matches!(
                NativeFittedEditIntent::decode_retained_fixed_scalars(
                    &malformed,
                    &mut ResourceBudget::new(0, 1_000_000, 1_000_000),
                    &|| false
                ),
                Err(NativeIntentError::InvalidRecord)
            ),
            "offset {offset}"
        );
    }
    let variable = NativeFittedIntentGroup::new_retained(
        "SingleValue",
        NativeFittedUpdates::Variable(Vec::new()),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        NativeFittedEditIntent::new_retained_fixed_scalars(
            "Measurements",
            vec![variable],
            &before,
            &mut ResourceBudget::new(0, 1_000_000, 1_000_000),
            &|| false
        ),
        Err(NativeIntentError::InvalidRecord)
    ));
    let mut nan = wire.clone();
    nan[carrier + 8..carrier + 12].copy_from_slice(&0x7fc0_0001_u32.to_le_bytes());
    assert!(matches!(
        NativeFittedEditIntent::decode_retained_fixed_scalars(&nan, &mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord)
    ));
    for value in [
        Value::Single(Single::from_bits(0x7f80_0000)),
        Value::Double(Double::from_bits(0x7ff0_0000_0000_0000)),
        Value::Double(Double::from_bits(0x7ff8_0000_0000_0001)),
        Value::Boolean(false),
        Value::Int32(1),
    ] {
        assert!(matches!(
            NativeFittedIntentGroup::new_retained_fixed_scalars(
                "SingleValue",
                NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(0, Value::Null, value)]),
                &mut budget(),
                &|| false
            ),
            Err(NativeIntentError::InvalidRecord)
        ));
    }
    assert!(
        NativeFittedEditIntent::decode_retained_fixed_scalars(&wire, &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        NativeFittedEditIntent::decode_retained_fixed_scalars(
            &wire,
            &mut ResourceBudget::new(0, 1_000_000, 1_000_000),
            &|| false
        )
        .is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn fixed_scalar_data_without_creation_feature_has_no_native_recovery_authority() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 4096];
    let wire = literal(&before, "ByteValue", 8, &[0], 8, &[1]);
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
            .qualify_retained_recovery(checked, CatalogScope::new(211), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!(target.writes, 0);
}
