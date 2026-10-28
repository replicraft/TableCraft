//! Independent selection/cache controls over original stored-cache literals.
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
use base as fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[allow(dead_code)]
#[path = "required_index_calculations/fixture.rs"]
mod indexed_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachment_modern/fixture.rs"]
mod modern_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod old_attachment;
use tablecraft_testkit::access as support;

use std::cell::Cell;
use std::io::Cursor;
use std::ops::Bound;
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, Source, prepare, source::ScanConstraint};
use tablecraft_sql::parse_query;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

fn run(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], Default::default())?
        .rows()
        .to_vec())
}

fn required(bytes: Vec<u8>, maximum_rows: u32) -> NativeSource<'static, Cursor<Vec<u8>>> {
    NativeSource::new_with_snapshot_limits(
        Cursor::new(bytes),
        CatalogScope::new(1),
        budget(),
        &|| false,
        TableSnapshotLimits::new(maximum_rows, 1).unwrap(),
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_index("Items", "IdOrder")
    .unwrap()
}

#[test]
fn required_small_selection_succeeds_above_the_complete_snapshot_cap() {
    for code in [5, 6] {
        for descending in [false, true] {
            let source = required(indexed_fixture::plain(code, descending, true), 1);
            assert_eq!(
                run(&source, "SELECT Id,Score FROM Items WHERE Id=20").unwrap(),
                [vec![Value::Int32(20), Value::Int32(-7)]]
            );
            assert_eq!(
                run(&source, "SELECT Score FROM Items WHERE Id=30").unwrap(),
                [vec![Value::Int32(999)]]
            );
            assert_eq!(
                run(&source, "SELECT Score FROM Items WHERE Id=999").unwrap(),
                Vec::<Vec<Value>>::new()
            );
            assert_eq!(
                run(&source, "SELECT Score FROM Items").err(),
                Some(QueryError::StructureLimit)
            );
            assert_eq!(
                run(&source, "SELECT Score FROM Items WHERE Id=10").err(),
                Some(QueryError::StructureLimit)
            );
            assert_eq!(
                run(&source, "SELECT Score FROM Items WHERE Id=20").unwrap(),
                [vec![Value::Int32(-7)]]
            );
        }
    }
}

#[test]
fn partial_cached_projection_is_not_reused_as_a_complete_or_different_selection() {
    let source = required(indexed_fixture::plain(6, false, true), 10);
    let constraint = ScanConstraint::Int32 {
        column: 0,
        lower: Bound::Included(20),
        upper: Bound::Included(20),
    };
    let mut selected = Vec::new();
    source
        .scan_projected("Items", &[1], Some(constraint), &mut |row| {
            selected.push(row.get(0).unwrap().clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(selected, [Value::Int32(-7)]);
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=10").unwrap(),
        [vec![Value::Null], vec![Value::Int32(41)]]
    );
    assert_eq!(
        run(&source, "SELECT Score FROM Items").unwrap(),
        [
            vec![Value::Null],
            vec![Value::Int32(41)],
            vec![Value::Int32(-7)],
            vec![Value::Int32(999)]
        ]
    );
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=30").unwrap(),
        [vec![Value::Int32(999)]]
    );
}

#[test]
fn selected_cache_validation_does_not_decode_unselected_forwarded_payloads() {
    let mut bytes = indexed_fixture::plain(6, false, true);
    let start = indexed_fixture::row_start(&bytes, 26, 0);
    bytes[start + 6] = 1; // Bad wrapper only on the forwarded Id10 root.
    let source = required(bytes, 10);
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=20").unwrap(),
        [vec![Value::Int32(-7)]]
    );
    let callbacks = Cell::new(0);
    let constraint = ScanConstraint::Int32 {
        column: 0,
        lower: Bound::Included(10),
        upper: Bound::Included(10),
    };
    for _ in 0..2 {
        assert!(
            source
                .scan_projected("Items", &[1], Some(constraint), &mut |_| {
                    callbacks.set(callbacks.get() + 1);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(callbacks.get(), 0);
    }
    assert_eq!(
        run(&source, "SELECT Id FROM Items WHERE Id=10").unwrap(),
        [vec![Value::Int32(10)], vec![Value::Int32(10)]]
    );
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=30").unwrap(),
        [vec![Value::Int32(999)]]
    );
}

#[test]
fn automatic_descending_index_returns_stored_values_in_original_table_order() {
    for code in [5, 6] {
        let source = NativeSource::new_with_snapshot_limits(
            Cursor::new(indexed_fixture::plain(code, true, true)),
            CatalogScope::new(1),
            budget(),
            &|| false,
            TableSnapshotLimits::new(3, 1).unwrap(),
        )
        .unwrap()
        .with_stored_calculations()
        .unwrap()
        .with_auto_indexes();
        assert_eq!(
            run(&source, "SELECT Id,Score FROM Items WHERE Id<=20").unwrap(),
            [
                vec![Value::Int32(10), Value::Null],
                vec![Value::Int32(10), Value::Int32(41)],
                vec![Value::Int32(20), Value::Int32(-7)],
            ]
        );
        let source = required(indexed_fixture::plain(code, true, true), 3);
        assert_eq!(
            run(&source, "SELECT Id,Score FROM Items WHERE Id<=20").unwrap(),
            [
                vec![Value::Int32(20), Value::Int32(-7)],
                vec![Value::Int32(10), Value::Null],
                vec![Value::Int32(10), Value::Int32(41)],
            ]
        );
    }
}

#[test]
fn residual_predicates_and_eager_failures_remain_authoritative_after_selection() {
    let source = required(indexed_fixture::plain(6, false, true), 10);
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=10 AND Score>0").unwrap(),
        [vec![Value::Int32(41)]]
    );
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=20 AND Score>0").unwrap(),
        Vec::<Vec<Value>>::new()
    );
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=999 AND 1/0=0").err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn cancellation_and_cumulative_native_quotas_do_not_publish_partial_selections() {
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let source = NativeSource::new(
        Cursor::new(indexed_fixture::plain(6, false, true)),
        CatalogScope::new(1),
        budget(),
        &cancel,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_index("Items", "IdOrder")
    .unwrap();
    source.columns("Items").unwrap();
    cancelled.set(true);
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=20").err(),
        Some(QueryError::Cancelled)
    );
    cancelled.set(false);
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=20").unwrap(),
        [vec![Value::Int32(-7)]]
    );

    let source = NativeSource::new(
        Cursor::new(indexed_fixture::plain(6, false, true)),
        CatalogScope::new(1),
        ResourceBudget::new(512 * 1024, 16384, 1_000_000),
        &|| false,
    )
    .unwrap()
    .with_stored_calculations()
    .unwrap()
    .with_index("Items", "IdOrder")
    .unwrap();
    assert_eq!(
        run(&source, "SELECT Score FROM Items WHERE Id=20").unwrap(),
        [vec![Value::Int32(-7)]]
    );
    let mut exhausted = false;
    for _ in 0..256 {
        match run(&source, "SELECT Score FROM Items WHERE Id=20") {
            Ok(rows) => assert_eq!(rows, [vec![Value::Int32(-7)]]),
            Err(QueryError::Limit(_)) => {
                exhausted = true;
                break;
            }
            Err(error) => panic!("unexpected selection error: {error}"),
        }
    }
    assert!(
        exhausted,
        "fresh partial reads must consume the shared native allowance"
    );
}
