//! Indexed originals keep independent framing and cannot borrow old write authority.
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
        NativeFittedIntentGroup::new_retained_indexed_long(
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
    let mut wire = b"TCNI0026".to_vec();
    wire.extend((total as u32).to_le_bytes());
    wire.extend([26, 0, 0, 0]);
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

fn cells() -> [Cell; 2] {
    [
        Cell {
            row: 0,
            expected: None,
            value: Some(7),
        },
        Cell {
            row: 5,
            expected: Some(7),
            value: None,
        },
    ]
}

#[test]
fn literal_indexed_long_originals_keep_zero_one_many_and_exact_profile() {
    let before = [0x26; 4096];
    let cells = cells();
    for selected in [&cells[..0], &cells[..1], cells.as_slice()] {
        let wire = literal(&before, "Archive", "CopiedScore", selected);
        let original = NativeFittedEditIntent::new_retained_indexed_long(
            "Archive",
            groups("CopiedScore", selected),
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            original
                .encode_retained_indexed_long(&mut budget(), &|| false)
                .unwrap(),
            wire
        );
        let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.profile(),
            NativeUpdateProfile::Ace16RetainedIndexedLongRows
        );
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        for decode in [
            NativeFittedEditIntent::decode_retained,
            NativeFittedEditIntent::decode_retained_long_column_batch,
            NativeFittedEditIntent::decode_retained_empty,
            NativeFittedEditIntent::decode_retained_wide_values,
        ] {
            assert!(decode(&wire, &mut budget(), &|| false).is_err());
        }
    }
}

#[test]
fn indexed_long_wire_refuses_duplicate_wrong_carriers_reserved_flags_and_truncation() {
    let before = [0x26; 4096];
    let cells = cells();
    let wire = literal(&before, "Archive", "CopiedScore", &cells);
    for end in 0..wire.len() {
        assert!(
            NativeFittedEditIntent::decode_retained_indexed_long(
                &wire[..end],
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    for (offset, byte) in [(0, b'X'), (12, 25), (14, 1), (60, 2), (64, 1), (68, 1)] {
        let mut wrong = wire.to_vec();
        wrong[offset] = byte;
        assert!(
            NativeFittedEditIntent::decode_retained_indexed_long(&wrong, &mut budget(), &|| false)
                .is_err(),
            "offset {offset}"
        );
    }
    assert!(
        NativeFittedEditIntent::decode_retained_indexed_long(
            &literal(&before, "Archive", "CopiedScore", &[cells[0], cells[0]]),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for updates in [
        NativeFittedUpdates::Variable(Vec::new()),
        NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
            0,
            Value::Null,
            Value::Int64(7),
        )]),
    ] {
        assert!(
            NativeFittedIntentGroup::new_retained_indexed_long(
                "CopiedScore",
                updates,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mut two = groups("CopiedScore", &cells);
    two.extend(groups("Other", &cells));
    assert!(
        NativeFittedEditIntent::new_retained_indexed_long(
            "Archive",
            two,
            &before,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(
        NativeFittedEditIntent::decode_retained_indexed_long(&wire, &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        NativeFittedEditIntent::decode_retained_indexed_long(
            &wire,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_indexed_long_data_cannot_authorize_native_recovery() {
    use crate::{EditError, NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_io::journal;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::support::io::journal::MemoryStorage;

    let before = vec![0x26; 4096];
    let mut after = before.to_vec();
    after[3000] ^= 1;
    let wire = literal(&before, "Table1", "Score", &cells());
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
            CatalogScope::new(32600),
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
