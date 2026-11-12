//! Independent TCNI27 framing and immutable Currency policy boundaries.
#[cfg(feature = "ace16-create")]
mod recovery;

#[cfg(not(feature = "ace16-create"))]
use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, Value},
};
#[cfg(not(feature = "ace16-create"))]
use tablecraft_io::journal;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::currency::batch::Cell;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_testkit::support::io::journal::MemoryStorage;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 2_000_000, 1_000_000_000)
}

fn value(value: Option<i64>) -> Value {
    value.map_or(Value::Null, |value| {
        Value::Currency(Currency::from_scaled(value))
    })
}

fn groups(column: &str, cells: &[Cell]) -> Vec<NativeFittedIntentGroup> {
    let updates = cells
        .iter()
        .map(|cell| FixedScalarUpdate::new(cell.row, value(cell.expected), value(cell.value)))
        .collect();
    vec![
        NativeFittedIntentGroup::new_retained_currency_column_batch(
            column,
            NativeFittedUpdates::Fixed(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(before: &[u8], table: &str, column: &str, cells: &[Cell]) -> Vec<u8> {
    let updates = cells
        .iter()
        .map(|cell| {
            28 + 8 * (usize::from(cell.expected.is_some()) + usize::from(cell.value.is_some()))
        })
        .sum::<usize>();
    let group = 20 + column.len() + updates;
    let total = 72 + table.len() + group;
    let mut wire = b"TCNI0027".to_vec();
    wire.extend((total as u32).to_le_bytes());
    wire.extend([27, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    wire.extend(digest.finalize());
    for word in [table.len() as u32, 1, cells.len() as u32, 0] {
        wire.extend(word.to_le_bytes());
    }
    wire.extend(table.as_bytes());
    for word in [group as u32, column.len() as u32, cells.len() as u32, 0, 0] {
        wire.extend(word.to_le_bytes());
    }
    wire.extend(column.as_bytes());
    for cell in cells {
        let length =
            28 + 8 * (u32::from(cell.expected.is_some()) + u32::from(cell.value.is_some()));
        wire.extend(length.to_le_bytes());
        wire.extend((cell.row as u64).to_le_bytes());
        for value in [cell.expected, cell.value] {
            wire.push(if value.is_some() { 4 } else { 0 });
            wire.extend([0; 3]);
            wire.extend(if value.is_some() { 8_u32 } else { 0_u32 }.to_le_bytes());
            if let Some(value) = value {
                wire.extend(value.to_le_bytes());
            }
        }
    }
    assert_eq!(wire.len(), total);
    wire
}

#[test]
fn currency_original_matches_literal_scaled_extrema_null_and_empty_records() {
    let before = vec![0x27; 4096];
    for cells in [
        Vec::new(),
        vec![
            Cell {
                row: 0,
                expected: Some(i64::MIN),
                value: Some(i64::MAX),
            },
            Cell {
                row: 1,
                expected: None,
                value: Some(-1),
            },
        ],
    ] {
        let original = NativeFittedEditIntent::new_retained_currency_column_batch(
            "T",
            groups("Amount", &cells),
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let bytes = literal(&before, "T", "Amount", &cells);
        assert_eq!(
            original
                .encode_retained_currency_column_batch(&mut budget(), &|| false)
                .unwrap(),
            bytes
        );
        assert!(
            original
                .encode_retained_long_column_batch(&mut budget(), &|| false)
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
            NativeUpdateProfile::Ace16RetainedCurrencyColumnBatch
        );
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), bytes);
        for decode in [
            NativeFittedEditIntent::decode_retained_currency,
            NativeFittedEditIntent::decode_retained_long_column_batch,
            NativeFittedEditIntent::decode_retained_indexed_long,
        ] {
            assert!(decode(&bytes, &mut budget(), &|| false).is_err());
        }
    }
}

#[test]
fn exact_scalar_grammar_wrong_family_and_framing_refuse_before_native_authority() {
    let before = vec![0x27; 4096];
    for wrong in [
        Value::Int64(12345),
        Value::Int32(1),
        Value::Double(Double::from_float(1.25)),
        Value::Decimal(Decimal::new(12345, 4).unwrap()),
    ] {
        let updates = vec![
            FixedScalarUpdate::new(0, Value::Null, wrong),
            FixedScalarUpdate::new(1, Value::Null, Value::Null),
        ];
        assert!(
            NativeFittedIntentGroup::new_retained_currency_column_batch(
                "Amount",
                NativeFittedUpdates::Fixed(updates),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let nulls = [
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
    ];
    let old_group = NativeFittedIntentGroup::new_retained_long_column_batch(
        "Amount",
        NativeFittedUpdates::Fixed(vec![
            FixedScalarUpdate::new(0, Value::Null, Value::Null),
            FixedScalarUpdate::new(1, Value::Null, Value::Null),
        ]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let old = NativeFittedEditIntent::new_retained_long_column_batch(
        "T",
        vec![old_group],
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        NativeUpdateIntent::from_retained_currency_column_batch(old),
        Err(NativeIntentError::UnsupportedProfile)
    ));
    let bytes = literal(&before, "T", "Amount", &nulls);
    for end in 0..bytes.len() {
        assert!(NativeUpdateIntent::decode(&bytes[..end], &mut budget(), &|| false).is_err());
    }
    for (offset, value) in [
        (0, b'X'),
        (12, 26),
        (14, 1),
        (60, 2),
        (64, 1),
        (68, 1),
        (85, 1),
        (89, 1),
    ] {
        let mut invalid = bytes.to_vec();
        invalid[offset] = value;
        assert!(
            NativeUpdateIntent::decode(&invalid, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    let typed = [
        Cell {
            row: 0,
            expected: Some(12345),
            value: None,
        },
        Cell {
            row: 1,
            expected: None,
            value: Some(-1),
        },
    ];
    let mut alias = literal(&before, "T", "Amount", &typed);
    assert_eq!(alias[111], 4);
    alias[111] = 3; // Same eight payload bytes, forbidden Int64 carrier tag.
    assert!(NativeUpdateIntent::decode(&alias, &mut budget(), &|| false).is_err());

    let singleton = literal(&before, "T", "Amount", &nulls[..1]);
    assert!(NativeUpdateIntent::decode(&singleton, &mut budget(), &|| false).is_err());
    assert!(NativeUpdateIntent::decode(&bytes, &mut budget(), &|| true).is_err());
    assert!(
        NativeUpdateIntent::decode(&bytes, &mut ResourceBudget::new(0, 0, 0), &|| false).is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_currency_data_retains_typed_native_source_refusal() {
    let before = vec![0x27; 4096];
    let mut after = before.to_vec();
    after[3000] ^= 1;
    let cells = [
        Cell {
            row: 0,
            expected: None,
            value: Some(1),
        },
        Cell {
            row: 1,
            expected: None,
            value: Some(2),
        },
    ];
    let original = NativeUpdateIntent::decode(
        &literal(&before, "T", "Amount", &cells),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        original.qualify_retained_recovery(
            checked,
            CatalogScope::new(4327),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
