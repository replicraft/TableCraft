//! TCNI20 Boolean witnesses remain exact data, separate from native bit authority.
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};

#[cfg(feature = "ace16-create")]
mod recovery;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn groups(column: &str, row: usize, expected: bool, value: Value) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained(
            column,
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                row,
                Value::Boolean(expected),
                value,
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(
    before: &[u8],
    table: &str,
    column: &str,
    row: u64,
    expected: bool,
    value: bool,
) -> Vec<u8> {
    let mut wire = b"TCNI0020".to_vec();
    wire.extend((122_u32 + table.len() as u32 + column.len() as u32).to_le_bytes());
    wire.extend([20, 0, 0, 0]);
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
    wire.extend((50_u32 + column.len() as u32).to_le_bytes());
    wire.extend((column.len() as u32).to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([0; 8]);
    wire.extend(column.as_bytes());
    wire.extend(30_u32.to_le_bytes());
    wire.extend(row.to_le_bytes());
    for bit in [expected, value] {
        wire.extend([1, 0, 0, 0]);
        wire.extend(1_u32.to_le_bytes());
        wire.push(u8::from(bit));
    }
    wire
}

#[test]
fn boolean_original_wire_keeps_true_false_canonical_and_older_profiles_closed() {
    let before = [0x27; 4096];
    for (expected, value) in [(false, true), (true, false), (false, false)] {
        let wire = literal(&before, "Table1", "Flag", 70_000, expected, value);
        let original = NativeFittedEditIntent::new_retained(
            "Table1",
            groups("Flag", 70_000, expected, Value::Boolean(value)),
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            original
                .encode_retained_boolean(&mut budget(), &|| false)
                .unwrap(),
            wire
        );
        let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.profile(),
            NativeUpdateProfile::Ace16RetainedBooleanRows
        );
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
        for decode in [
            NativeFittedEditIntent::decode,
            NativeFittedEditIntent::decode_retained,
            NativeFittedEditIntent::decode_retained_opaque,
            NativeFittedEditIntent::decode_retained_currency,
        ] {
            assert!(decode(&wire, &mut budget(), &|| false).is_err());
        }
        let mut invalid_boolean = wire.clone();
        *invalid_boolean.last_mut().unwrap() = 2;
        assert!(matches!(
            NativeFittedEditIntent::decode_retained_boolean(
                &invalid_boolean,
                &mut budget(),
                &|| false
            ),
            Err(NativeIntentError::InvalidRecord)
        ));
        for offset in [8, 14, 16, 68] {
            let mut invalid = wire.clone();
            invalid[offset] ^= 1;
            assert!(
                matches!(
                    NativeFittedEditIntent::decode_retained_boolean(
                        &invalid,
                        &mut ResourceBudget::new(0, 100, 100_000),
                        &|| false
                    ),
                    Err(NativeIntentError::InvalidRecord)
                ),
                "offset {offset}"
            );
        }
    }
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn boolean_journal_data_cannot_enable_native_bit_mutation() {
    use crate::{EditError, NativeRecoveryQualificationError};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0; 4096];
    let wire = literal(&before, "Table1", "Flag", 0, false, true);
    let intent = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
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
        intent
            .qualify_retained_recovery(checked, CatalogScope::new(1740), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(_)
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
