use super::*;
use tablecraft_ace::{read_table_batched, read_table_projection_batched};

#[test]
fn batched_snapshots_match_independent_sequences_at_batch_and_empty_boundaries() {
    for count in [0, 1, 255, 256, 257, 600, 16_384] {
        let bytes = database(count);
        let snapshot = read_table_batched(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut ResourceBudget::new(64 * 1024 * 1024, 20_000, 1_000_000),
            &|| false,
        )
        .unwrap();
        assert_eq!(snapshot.rows(), expected(0, count));
        let empty = read_table_projection_batched(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &[],
            &mut ResourceBudget::new(64 * 1024 * 1024, 20_000, 1_000_000),
            &|| false,
        )
        .unwrap();
        assert_eq!(empty.rows().len(), count);
        assert!(empty.rows().iter().all(Vec::is_empty));
    }
    let mut input = PageReads::new(database(16_385));
    assert!(matches!(
        read_table_batched(&mut input, SCOPE, "Items", &mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    ));
    assert_eq!(input.count(FIRST_DATA_PAGE as u64), 0);
}

#[test]
fn final_cancellation_prevents_snapshot_publication_after_all_batches() {
    let bytes = database(600);
    let calls = Cell::new(0);
    read_table_batched(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let final_call = calls.get();
    calls.set(0);
    assert!(matches!(
        read_table_batched(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == final_call
            }
        ),
        Err(InspectError::Cancelled)
    ));
}
