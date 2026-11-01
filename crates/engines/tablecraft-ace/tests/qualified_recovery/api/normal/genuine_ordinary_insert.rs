//! Normal insertion commits match images qualified before the producer existed.
#[path = "genuine_ordinary_insert/fixtures.rs"]
mod fixtures;
#[path = "genuine_ordinary_insert/recovery.rs"]
mod recovery;
#[path = "genuine_ordinary_insert/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use fixtures::Case;
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    NativeImage, RecoverableEditError, RecoverableNativeInsertPlan, RowInsert, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeInsertPlan, RecoverableEditError> {
    image(&case.before).plan_recoverable_ordinary_row_inserts(
        "Sentinel",
        fixtures::requests(),
        work,
        cancelled,
    )
}

#[test]
#[ignore = "requires all four independently qualified ordinary INSERT images"]
fn complete_insert_commits_preserve_original_row_and_append_literal_visible_record() {
    let mut committed = 0;
    for case in fixtures::cases() {
        let planned = plan(&case, &mut budget(), &|| false).unwrap();
        assert_eq!(planned.before_bytes(), case.before);
        assert_eq!(planned.after_bytes(), case.after);
        assert_eq!(planned.inserted_rows(), 1);
        assert_eq!(
            planned.intent().encode(&mut budget(), &|| false).unwrap(),
            fixtures::wire(&case)
        );

        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            planned
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed)
        );
        let table = read_table(
            &mut Cursor::new(target.bytes.get_ref()),
            scope(),
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows().len(), 2);
        assert_eq!(
            table.rows()[1],
            tablecraft_testkit::access::genuine_ordinary_insert::values()
        );
        assert_eq!(
            table.rows()[0],
            [
                Value::Int32(23),
                Value::Text("unchanged sentinel".into()),
                Value::Binary(vec![199, 0, 255]),
            ]
        );
        committed += 1;
    }
    assert_eq!(committed, 2);
}

#[test]
#[ignore = "requires all four independently qualified ordinary INSERT images"]
fn empty_batches_remain_qualified_and_invalid_complete_rows_refuse_before_publication() {
    for case in fixtures::cases() {
        let empty = image(&case.before)
            .plan_recoverable_ordinary_row_inserts("Sentinel", vec![], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.before_bytes(), case.before);
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.inserted_rows(), 0);

        for values in [
            vec![],
            vec![Value::Int32(24)],
            vec![
                Value::Text("24".into()),
                Value::Text("new".into()),
                Value::Null,
            ],
            vec![Value::Int32(24), Value::Text("n".repeat(81)), Value::Null],
            vec![
                Value::Int32(24),
                Value::Text("new".into()),
                Value::Binary(vec![1]),
            ],
        ] {
            assert!(
                image(&case.before)
                    .plan_recoverable_ordinary_row_inserts(
                        "Sentinel",
                        vec![RowInsert::new(values)],
                        &mut budget(),
                        &|| false,
                    )
                    .is_err()
            );
        }
    }
}
