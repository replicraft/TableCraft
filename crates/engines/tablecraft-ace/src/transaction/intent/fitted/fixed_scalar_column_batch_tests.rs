//! Independent TCNI28 descriptors, literal framing and immutable older policies.
#[cfg(feature = "ace16-create")]
mod recovery;

#[cfg(not(feature = "ace16-create"))]
use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeFixedScalarKind, NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, LegacyDateTime, Single, Value},
};
#[cfg(not(feature = "ace16-create"))]
use tablecraft_io::journal;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fixed_scalars::{
    COLUMNS_TO_EDIT, Column, Scalar,
    batch::{self as raw, Cell},
};
#[cfg(not(feature = "ace16-create"))]
use tablecraft_testkit::support::io::journal::MemoryStorage;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 4_000_000, 1_000_000_000)
}

fn kind(column: Column) -> NativeFixedScalarKind {
    match column {
        Column::ByteValue => NativeFixedScalarKind::Byte,
        Column::ShortValue => NativeFixedScalarKind::Int16,
        Column::LargeValue => NativeFixedScalarKind::Int64,
        Column::SingleValue => NativeFixedScalarKind::Single,
        Column::DoubleValue => NativeFixedScalarKind::Double,
        Column::DateValue => NativeFixedScalarKind::DateTime,
    }
}

fn value(value: Option<Scalar>) -> Value {
    match value {
        None => Value::Null,
        Some(Scalar::Byte(value)) => Value::Byte(value),
        Some(Scalar::Int16(value)) => Value::Int16(value),
        Some(Scalar::Int64(value)) => Value::Int64(value),
        Some(Scalar::SingleBits(value)) => Value::Single(Single::from_bits(value)),
        Some(Scalar::DoubleBits(value)) => Value::Double(Double::from_bits(value)),
        Some(Scalar::DateTimeBits(value)) => {
            Value::DateTime(LegacyDateTime::from_bits(value).unwrap())
        }
    }
}

fn groups(column: Column, cells: &[Cell]) -> Vec<NativeFittedIntentGroup> {
    let updates = cells
        .iter()
        .map(|cell| FixedScalarUpdate::new(cell.row, value(cell.expected), value(cell.value)))
        .collect();
    vec![
        NativeFittedIntentGroup::new_retained_fixed_scalar_column_batch(
            column.name(),
            kind(column),
            NativeFittedUpdates::Fixed(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn payload(value: Option<Scalar>) -> (u8, Vec<u8>) {
    match value {
        None => (0, Vec::new()),
        Some(Scalar::Byte(value)) => (8, vec![value]),
        Some(Scalar::Int16(value)) => (9, value.to_le_bytes().to_vec()),
        Some(Scalar::Int64(value)) => (3, value.to_le_bytes().to_vec()),
        Some(Scalar::SingleBits(value)) => (10, value.to_le_bytes().to_vec()),
        Some(Scalar::DoubleBits(value)) => (11, value.to_le_bytes().to_vec()),
        Some(Scalar::DateTimeBits(value)) => (5, value.to_le_bytes().to_vec()),
    }
}

fn literal(before: &[u8], column: Column, cells: &[Cell]) -> Vec<u8> {
    let table = "Measurements";
    let name = column.name();
    let descriptor = match column {
        Column::ByteValue => 2,
        Column::ShortValue => 3,
        Column::LargeValue => 19,
        Column::SingleValue => 6,
        Column::DoubleValue => 7,
        Column::DateValue => 8,
    };
    let mut encoded = Vec::new();
    for cell in cells {
        let (expected_tag, expected) = payload(cell.expected);
        let (tag, value) = payload(cell.value);
        encoded.extend((28_u32 + expected.len() as u32 + value.len() as u32).to_le_bytes());
        encoded.extend((cell.row as u64).to_le_bytes());
        for (tag, bytes) in [(expected_tag, expected), (tag, value)] {
            encoded.extend([tag, 0, 0, 0]);
            encoded.extend((bytes.len() as u32).to_le_bytes());
            encoded.extend(bytes);
        }
    }
    let group = 20 + name.len() + encoded.len();
    let total = 72 + table.len() + group;
    let mut bytes = b"TCNI0028".to_vec();
    bytes.extend((total as u32).to_le_bytes());
    bytes.extend([28, 0, 0, 0]);
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    bytes.extend(digest.finalize());
    for word in [table.len() as u32, 1, cells.len() as u32, 0] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(table.as_bytes());
    for word in [group as u32, name.len() as u32, cells.len() as u32] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([0, descriptor, 0, 0]);
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(name.as_bytes());
    bytes.extend(encoded);
    assert_eq!(bytes.len(), total);
    bytes
}

#[test]
fn every_kind_matches_literal_payload_and_zero_null_originals() {
    let before = vec![0x28; 4096];
    for column in COLUMNS_TO_EDIT {
        for cells in [
            Vec::new(),
            vec![
                Cell {
                    row: 0,
                    expected: None,
                    value: None,
                },
                Cell {
                    row: 1,
                    expected: None,
                    value: None,
                },
            ],
            vec![
                Cell {
                    row: 0,
                    expected: Some(raw::initial(column)),
                    value: None,
                },
                Cell {
                    row: 1,
                    expected: None,
                    value: Some(raw::replacement(column)),
                },
            ],
        ] {
            let original = NativeFittedEditIntent::new_retained_fixed_scalar_column_batch(
                "Measurements",
                groups(column, &cells),
                &before,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let bytes = literal(&before, column, &cells);
            assert_eq!(
                original
                    .encode_retained_fixed_scalar_column_batch(&mut budget(), &|| false)
                    .unwrap(),
                bytes
            );
            assert!(
                original
                    .encode_retained_fixed_scalars(&mut budget(), &|| false)
                    .is_err()
            );
            assert!(
                original
                    .encode_retained_long_column_batch(&mut budget(), &|| false)
                    .is_err()
            );
            assert!(
                original
                    .encode_retained_currency_column_batch(&mut budget(), &|| false)
                    .is_err()
            );
            assert!(
                original
                    .encode_retained_indexed_long(&mut budget(), &|| false)
                    .is_err()
            );
            let decoded = NativeUpdateIntent::decode(&bytes, &mut budget(), &|| false).unwrap();
            assert_eq!(
                decoded.profile(),
                NativeUpdateProfile::Ace16RetainedFixedScalarColumnBatch
            );
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), bytes);
            for decode in [
                NativeFittedEditIntent::decode_retained_fixed_scalars,
                NativeFittedEditIntent::decode_retained_long_column_batch,
                NativeFittedEditIntent::decode_retained_currency_column_batch,
                NativeFittedEditIntent::decode_retained_indexed_long,
            ] {
                assert!(decode(&bytes, &mut budget(), &|| false).is_err());
            }
        }
    }
}

#[test]
fn descriptor_provenance_cannot_cross_old_constructors_even_for_null_and_empty() {
    let before = vec![0x28; 4096];
    for cells in [
        Vec::new(),
        vec![
            Cell {
                row: 0,
                expected: None,
                value: None,
            },
            Cell {
                row: 1,
                expected: None,
                value: None,
            },
        ],
    ] {
        for old in [
            NativeFittedEditIntent::new_retained_fixed_scalars,
            NativeFittedEditIntent::new_retained_long_column_batch,
            NativeFittedEditIntent::new_retained_currency_column_batch,
            NativeFittedEditIntent::new_retained_indexed_long,
            NativeFittedEditIntent::new_retained,
        ] {
            assert!(
                old(
                    "Measurements",
                    groups(Column::DoubleValue, &cells),
                    &before,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        }
        let old_updates = cells
            .iter()
            .map(|cell| FixedScalarUpdate::new(cell.row, Value::Null, Value::Null))
            .collect();
        let old_group = NativeFittedIntentGroup::new_retained_fixed_scalars(
            "DoubleValue",
            NativeFittedUpdates::Fixed(old_updates),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            NativeFittedEditIntent::new_retained_fixed_scalar_column_batch(
                "Measurements",
                vec![old_group],
                &before,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}

#[test]
fn malformed_descriptors_carriers_and_nonfinite_bits_refuse_in_preflight() {
    let before = vec![0x28; 4096];
    let cells = [
        Cell {
            row: 0,
            expected: None,
            value: Some(Scalar::DoubleBits(1)),
        },
        Cell {
            row: 1,
            expected: None,
            value: None,
        },
    ];
    let bytes = literal(&before, Column::DoubleValue, &cells);
    let group = 72 + "Measurements".len();
    let first = group + 20 + "DoubleValue".len();
    for (magic, profile) in [
        (b"TCNI0021", 21_u16),
        (b"TCNI0025", 25),
        (b"TCNI0026", 26),
        (b"TCNI0027", 27),
    ] {
        let mut invalid = bytes.to_vec();
        invalid[..8].copy_from_slice(magic);
        invalid[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(NativeUpdateIntent::decode(&invalid, &mut budget(), &|| false).is_err());
    }
    for end in 0..bytes.len() {
        assert!(NativeUpdateIntent::decode(&bytes[..end], &mut budget(), &|| false).is_err());
    }
    for (offset, replacement) in [
        (12, 27),
        (14, 1),
        (60, 2),
        (64, 1),
        (68, 1),
        (group + 13, 5),
        (group + 13, 19),
        (group + 14, 1),
        (group + 16, 1),
        (first + 20, 3),
    ] {
        let mut invalid = bytes.to_vec();
        invalid[offset] = replacement;
        assert!(
            NativeUpdateIntent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    for (column, invalid) in [
        (
            Column::SingleValue,
            Scalar::SingleBits(f32::INFINITY.to_bits()),
        ),
        (Column::SingleValue, Scalar::SingleBits(f32::NAN.to_bits())),
        (
            Column::DoubleValue,
            Scalar::DoubleBits(f64::NEG_INFINITY.to_bits()),
        ),
        (Column::DateValue, Scalar::DateTimeBits(f64::NAN.to_bits())),
    ] {
        let invalid_cells = [
            Cell {
                row: 0,
                expected: None,
                value: Some(invalid),
            },
            Cell {
                row: 1,
                expected: None,
                value: None,
            },
        ];
        assert!(
            NativeUpdateIntent::decode(
                &literal(&before, column, &invalid_cells),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mixed = NativeFittedUpdates::Fixed(vec![
        FixedScalarUpdate::new(0, Value::Double(Double::from_float(1.0)), Value::Null),
        FixedScalarUpdate::new(1, Value::Null, Value::Null),
    ]);
    assert!(
        NativeFittedIntentGroup::new_retained_fixed_scalar_column_batch(
            "DateValue",
            NativeFixedScalarKind::DateTime,
            mixed,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(
        NativeUpdateIntent::decode(
            &literal(&before, Column::DoubleValue, &cells[..1]),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(NativeUpdateIntent::decode(&bytes, &mut budget(), &|| true).is_err());
    assert!(
        NativeUpdateIntent::decode(&bytes, &mut ResourceBudget::new(0, 0, 0), &|| false).is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_typed_data_cannot_construct_native_recovery_authority() {
    let before = vec![0x28; 4096];
    let mut target = MemoryStorage::new(before.to_vec());
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
    let original = NativeUpdateIntent::decode(
        &literal(&before, Column::DoubleValue, &[]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        original.qualify_retained_recovery(
            checked,
            CatalogScope::new(4328),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!(target.writes, 0);
}
