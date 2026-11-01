//! Normal TCNA carries original NULL data; all images were qualified before producers.
#[path = "genuine_desc_nullable_insert/fixtures.rs"]
mod fixtures;
#[path = "genuine_desc_nullable_insert/recovery.rs"]
mod recovery;
#[path = "genuine_desc_nullable_insert/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use fixtures::cases;
use journals::Marker;
use sources::values;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, NativeImage, RecoverableEditError,
    RowInsert, inspect_table_index_counter, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::genuine_desc_nullable_insert as sources;

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn assert_rows(raw: &[u8], count: usize) {
    let sequential = read_table(
        &mut Cursor::new(raw),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sequential.rows(), sources::rows(count, false));
    for (name, physical, history) in [
        ("PrimaryID", 0, 3 + count),
        ("PrimaryAlias", 0, 3 + count),
        ("GroupOrder", 1, 3),
        ("GroupAlias", 1, 3),
    ] {
        let indexed = read_table_index(
            &mut Cursor::new(raw),
            scope(),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(indexed.rows(), sources::rows(count, physical == 1));
        let stored = inspect_table_index_counter(
            &mut Cursor::new(raw),
            scope(),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(stored.physical_index(), physical);
        assert_eq!(stored.value() as usize, history);
    }
}

fn commit(before: &[u8], expected: &[u8], original: Vec<Vec<Value>>) {
    let count = original.len();
    let plan = image(before)
        .plan_recoverable_ordinary_row_inserts(
            "Items",
            original.iter().cloned().map(RowInsert::new).collect(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(plan.inserted_rows(), count);
    assert_eq!(plan.intent().rows().len(), count);
    for (actual, requested) in plan.intent().rows().iter().zip(&original) {
        assert_eq!(actual.values(), requested);
    }
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        fixtures::wire(before, &original)
    );
    let mut target = Storage::new(before.to_vec());
    let mut journal = Storage::new(Vec::new());
    assert_eq!(
        plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .unwrap(),
        count
    );
    assert_eq!(target.bytes.get_ref(), expected);
    assert_eq!(
        journal.bytes.get_ref(),
        &journals::journal(before, expected, Marker::Committed)
    );
}

#[test]
#[ignore = "requires source native6 and SDK36 nullable/DESC INSERT qualification"]
fn nullable_desc_single_batch_and_sequential_commits_preserve_null_ties_and_histories() {
    let mut committed = 0;
    for case in cases() {
        assert_rows(&case.before, 0);
        for (count, expected) in [(1, &case.single), (2, &case.batch)] {
            commit(
                &case.before,
                expected,
                (0..count).map(|position| values(position == 1)).collect(),
            );
            assert_rows(expected, count);
            committed += 1;
        }
        // A fresh native snapshot of the literal single AFTER now contains two
        // stored NULL members. The second original request adds an existing
        // present Guard family; no extra history or changed ROW association.
        commit(&case.before, &case.single, vec![values(false)]);
        commit(&case.single, &case.batch, vec![values(true)]);
        assert_rows(&case.batch, 2);
        committed += 2;
    }
    assert_eq!(committed, 8);
}

#[test]
#[ignore = "requires source native6 and SDK36 nullable/DESC INSERT qualification"]
fn nullable_secondary_does_not_weaken_primary_unique_required_or_empty_admission() {
    for case in cases() {
        let empty = image(&case.before)
            .plan_recoverable_ordinary_row_inserts("Items", vec![], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.inserted_rows(), 0);
        assert_eq!(
            empty.intent().encode(&mut budget(), &|| false).unwrap(),
            fixtures::wire(&case.before, &[])
        );
        for (id, wanted) in [
            (Value::Int32(1), IndexConstraintViolation::DuplicateKey),
            (Value::Null, IndexConstraintViolation::NullKey),
        ] {
            let row = vec![id, Value::Int32(17), Value::Null];
            let error = image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    "Items",
                    vec![RowInsert::new(row)],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(
                matches!(error, RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(actual))) if actual == wanted)
            );
        }
    }
}
