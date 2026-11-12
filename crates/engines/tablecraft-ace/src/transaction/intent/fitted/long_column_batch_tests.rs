//! Explicit multi-row LONG originals retain their own grammar and source binding.
#[cfg(feature = "ace16-create")]
mod recovery;

use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};

#[derive(Clone, Copy)]
struct Cell {
    row: usize,
    expected: Option<i32>,
    value: Option<i32>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn value(value: Option<i32>) -> Value {
    value.map_or(Value::Null, Value::Int32)
}

fn groups(column: &str, cells: &[Cell]) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained_long_column_batch(
            column,
            NativeFittedUpdates::Fixed(
                cells
                    .iter()
                    .map(|cell| {
                        FixedScalarUpdate::new(cell.row, value(cell.expected), value(cell.value))
                    })
                    .collect(),
            ),
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
            28 + 4 * (usize::from(cell.expected.is_some()) + usize::from(cell.value.is_some()))
        })
        .sum::<usize>();
    let group = 20 + column.len() + updates;
    let total = 72 + table.len() + group;
    let mut wire = b"TCNI0025".to_vec();
    wire.extend((total as u32).to_le_bytes());
    wire.extend([25, 0, 0, 0]);
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
            28 + 4 * (u32::from(cell.expected.is_some()) + u32::from(cell.value.is_some()));
        wire.extend(length.to_le_bytes());
        wire.extend((cell.row as u64).to_le_bytes());
        for value in [cell.expected, cell.value] {
            wire.extend([if value.is_some() { 2 } else { 0 }, 0, 0, 0]);
            wire.extend(if value.is_some() { 4_u32 } else { 0 }.to_le_bytes());
            if let Some(value) = value {
                wire.extend(value.to_le_bytes());
            }
        }
    }
    assert_eq!(wire.len(), total);
    wire
}

fn data_cells() -> [Cell; 2] {
    [
        Cell {
            row: 70_000,
            expected: Some(i32::MIN),
            value: None,
        },
        Cell {
            row: 2_000_000,
            expected: None,
            value: Some(i32::MAX),
        },
    ]
}

#[test]
fn literal_long_batch_preserves_original_ordinals_nulls_and_exact_action_identity() {
    let before = [0x25; 4096];
    let cells = data_cells();
    let wire = literal(&before, "Table1", "Score", &cells);
    let original = NativeFittedEditIntent::new_retained_long_column_batch(
        "Table1",
        groups("Score", &cells),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        original
            .encode_retained_long_column_batch(&mut budget(), &|| false)
            .unwrap(),
        wire
    );
    let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16RetainedLongColumnBatch
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    for decode in [
        NativeFittedEditIntent::decode_retained,
        NativeFittedEditIntent::decode_retained_opaque,
        NativeFittedEditIntent::decode_retained_currency,
        NativeFittedEditIntent::decode_retained_boolean,
        NativeFittedEditIntent::decode_retained_fixed_scalars,
        NativeFittedEditIntent::decode_retained_wide_values,
        NativeFittedEditIntent::decode_retained_boolean_materialization,
        NativeFittedEditIntent::decode_retained_empty,
    ] {
        assert!(decode(&wire, &mut budget(), &|| false).is_err());
    }
    let mut stale = before;
    stale[3000] ^= 1;
    assert!(
        decoded
            .verify_before(&stale, &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn long_batch_grammar_refuses_small_duplicate_variable_wrong_carrier_and_malformed_requests() {
    let before = [0x25; 4096];
    let cells = data_cells();
    for selected in [&cells[..0], &cells[..1]] {
        assert!(
            NativeFittedEditIntent::decode_retained_long_column_batch(
                &literal(&before, "Table1", "Score", selected),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let duplicate = [cells[0], cells[0]];
    assert!(
        NativeFittedEditIntent::decode_retained_long_column_batch(
            &literal(&before, "Table1", "Score", &duplicate),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for updates in [
        NativeFittedUpdates::Variable(Vec::new()),
        NativeFittedUpdates::Fixed(Vec::new()),
        NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(0, Value::Null, Value::Null)]),
        NativeFittedUpdates::Fixed(vec![
            FixedScalarUpdate::new(0, Value::Null, Value::Boolean(false)),
            FixedScalarUpdate::new(1, Value::Null, Value::Int32(1)),
        ]),
    ] {
        assert!(
            NativeFittedIntentGroup::new_retained_long_column_batch(
                "Score",
                updates,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mut multiple = groups("Score", &cells);
    multiple.extend(groups("Other", &cells));
    assert!(
        NativeFittedEditIntent::new_retained_long_column_batch(
            "Table1",
            multiple,
            &before,
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
    assert!(
        NativeFittedEditIntent::new_retained_long_column_batch(
            "Table1",
            Vec::new(),
            &before,
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
    let wire = literal(&before, "Table1", "Score", &cells);
    for end in 0..wire.len() {
        assert!(
            NativeFittedEditIntent::decode_retained_long_column_batch(
                &wire[..end],
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    for (offset, byte) in [
        (0, b'X'),
        (12, 24),
        (14, 1),
        (60, 2),
        (64, 1),
        (68, 1),
        (90, 1),
        (94, 1),
        (115, 1),
    ] {
        let mut wrong = wire.to_vec();
        wrong[offset] = byte;
        assert!(
            NativeFittedEditIntent::decode_retained_long_column_batch(
                &wrong,
                &mut budget(),
                &|| false
            )
            .is_err(),
            "offset {offset}"
        );
    }
    assert!(
        NativeFittedEditIntent::decode_retained_long_column_batch(&wire, &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        NativeFittedEditIntent::decode_retained_long_column_batch(
            &wire,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_long_batch_data_cannot_authorize_native_recovery() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0x25; 4096];
    let mut after = before.to_vec();
    after[3000] ^= 1;
    let wire = literal(&before, "Table1", "Score", &data_cells());
    let original = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
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
            CatalogScope::new(32500),
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
