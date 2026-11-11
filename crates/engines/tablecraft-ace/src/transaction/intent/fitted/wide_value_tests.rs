//! Closed GUID/Decimal original carriers retain exact values without native authority.
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeIntentError, NativeUpdateIntent, NativeUpdateProfile, SlotScalarUpdate,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Guid, Value},
};

#[cfg(feature = "ace16-create")]
mod recovery;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 512 * 1024 * 1024)
}

fn groups(column: &str, row: usize, expected: Value, value: Value) -> Vec<NativeFittedIntentGroup> {
    let updates = if column == "Token" {
        NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(row, expected, value)])
    } else {
        NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(row, expected, value)])
    };
    vec![
        NativeFittedIntentGroup::new_retained_wide_values(column, updates, &mut budget(), &|| {
            false
        })
        .unwrap(),
    ]
}

fn literal(before: &[u8], column: &str, expected: (u8, &[u8]), value: (u8, &[u8])) -> Vec<u8> {
    let table = "ExactValues";
    let cell = 28 + expected.1.len() + value.1.len();
    let group = 20 + column.len() + cell;
    let total = 72 + table.len() + group;
    let mut bytes = b"TCNI0022".to_vec();
    bytes.extend((total as u32).to_le_bytes());
    bytes.extend([22, 0, 0, 0]);
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    bytes.extend(hash.finalize());
    for word in [table.len() as u32, 1, 1, 0] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(table.as_bytes());
    for word in [group as u32, column.len() as u32, 1] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([u8::from(column == "Amount"), 0, 0, 0]);
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(column.as_bytes());
    bytes.extend((cell as u32).to_le_bytes());
    bytes.extend(70_000_u64.to_le_bytes());
    for (tag, payload) in [expected, value] {
        bytes.extend([tag, 0, 0, 0]);
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
    }
    bytes
}

#[test]
fn canonical_guid_and_exact_decimal_match_independent_wire_and_keep_old_grammars_closed() {
    let before = [0x56; 4096];
    let token = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let mut decimal = 90_071_992_547_409_931_234_i128.to_le_bytes().to_vec();
    decimal.push(4);
    for (column, tag, value, payload) in [
        (
            "Token",
            12,
            Value::Guid(Guid::from_bytes(token)),
            token.to_vec(),
        ),
        (
            "Amount",
            13,
            Value::Decimal(Decimal::new(90_071_992_547_409_931_234, 4).unwrap()),
            decimal,
        ),
    ] {
        for (expected, replacement, left, right) in [
            (
                Value::Null,
                value.clone(),
                (0, &[][..]),
                (tag, payload.as_slice()),
            ),
            (value, Value::Null, (tag, payload.as_slice()), (0, &[][..])),
            (Value::Null, Value::Null, (0, &[][..]), (0, &[][..])),
        ] {
            let wire = literal(&before, column, left, right);
            let original = NativeFittedEditIntent::new_retained_wide_values(
                "ExactValues",
                groups(column, 70_000, expected, replacement),
                &before,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                original
                    .encode_retained_wide_values(&mut budget(), &|| false)
                    .unwrap(),
                wire
            );
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(
                decoded.profile(),
                NativeUpdateProfile::Ace16RetainedWideValueRows
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
            ] {
                assert!(decode(&wire, &mut budget(), &|| false).is_err());
            }
        }
    }
}

#[test]
fn wide_carriers_reject_wrong_families_widths_scale_and_explicit_budget_exhaustion() {
    let before = [0; 4096];
    let wire = literal(&before, "Token", (12, &[0; 16]), (0, &[]));
    let group = 72 + "ExactValues".len();
    let carrier = group + 20 + "Token".len() + 12;
    for (offset, value) in [
        (8, 0),
        (14, 1),
        (68, 1),
        (group + 12, 1),
        (carrier, 13),
        (carrier + 4, 15),
    ] {
        let mut malformed = wire.clone();
        malformed[offset] = value;
        assert!(
            matches!(
                NativeFittedEditIntent::decode_retained_wide_values(
                    &malformed,
                    &mut budget(),
                    &|| false
                ),
                Err(NativeIntentError::InvalidRecord)
            ),
            "offset {offset}"
        );
    }
    let mut decimal = [0; 17];
    decimal[16] = 29;
    let malformed = literal(&before, "Amount", (13, &decimal), (0, &[]));
    assert!(matches!(
        NativeFittedEditIntent::decode_retained_wide_values(&malformed, &mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord)
    ));
    for updates in [
        NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
            0,
            Value::Null,
            Value::Decimal(Decimal::new(1, 0).unwrap()),
        )]),
        NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
            0,
            Value::Null,
            Value::Guid(Guid::from_bytes([0; 16])),
        )]),
    ] {
        assert!(matches!(
            NativeFittedIntentGroup::new_retained_wide_values(
                "Bad",
                updates,
                &mut budget(),
                &|| false
            ),
            Err(NativeIntentError::InvalidRecord)
        ));
    }
    assert!(
        NativeFittedEditIntent::decode_retained_wide_values(&wire, &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        NativeFittedEditIntent::decode_retained_wide_values(
            &wire,
            &mut ResourceBudget::new(1, 1_000_000, 1_000_000),
            &|| false
        )
        .is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_data_has_no_native_wide_value_recovery_authority() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;
    let before = vec![0; 4096];
    let wire = literal(&before, "Token", (0, &[]), (12, &[0; 16]));
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
            .qualify_retained_recovery(checked, CatalogScope::new(221), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!(target.writes, 0);
}
