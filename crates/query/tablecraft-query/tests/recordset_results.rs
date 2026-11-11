#[path = "recordset_results/mixed.rs"]
mod mixed;
#[path = "recordset_results/source.rs"]
mod source;

use source::Data;
use std::{cell::Cell, sync::Arc};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_db::{
    MemberAvailability, OwnedRecordset, RecordsetKind, RecordsetPresence, StoredCell,
};
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, OutputType, QueryError, QueryLimits, QueryValue,
    Source, prepare,
};
use tablecraft_sql::parse_query;

fn run(sql: &str, data: &dyn Source) -> Result<CollectionQueryResult, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), data)?.execute_collections(data, &[], ExecutionOptions::default())
}

fn scalar(value: i32) -> QueryValue {
    QueryValue::Scalar(ExpressionValue::Value(Value::Int32(value)))
}

fn recordset(value: &QueryValue) -> &Arc<OwnedRecordset> {
    let QueryValue::Recordset(value) = value else {
        panic!("expected recordset")
    };
    value
}

#[test]
fn bare_and_wildcard_projections_preserve_four_parent_states_without_loading_file_data() {
    let data = Data::new();
    for sql in [
        "SELECT Id,Files FROM T ORDER BY Id",
        "SELECT * FROM T ORDER BY Id",
    ] {
        let result = run(sql, &data).unwrap();
        assert_eq!(result.columns(), ["Id", "Files"]);
        assert_eq!(result.rows().len(), 4);
        assert_eq!(
            result.output_types()[1],
            OutputType::Recordset {
                parent_nullable: false
            }
        );
        let schema = result.recordset_schema(1).unwrap();
        assert_eq!(schema.columns()[0].name(), "FileName");
        assert_eq!(
            schema.availability(),
            [
                MemberAvailability::Available,
                MemberAvailability::Uninspected
            ]
        );
        for (index, row) in result.rows().iter().enumerate() {
            assert_eq!(row[0], scalar(index as i32 + 1));
            assert!(Arc::ptr_eq(recordset(&row[1]), &data.records[index]));
        }
        let first = recordset(&result.rows()[0][1]);
        assert_eq!(
            first.rows()[0][0],
            StoredCell::Value(Value::Text("named.txt".into()))
        );
        assert_eq!(first.rows()[1][0], StoredCell::Value(Value::Null));
        assert_eq!(first.rows()[0][1], StoredCell::Uninspected);
        assert_eq!(
            recordset(&result.rows()[1][1]).rows()[0][0],
            StoredCell::Value(Value::Text(String::new()))
        );
        assert_eq!(
            recordset(&result.rows()[2][1]).presence(),
            RecordsetPresence::Assigned
        );
        assert_eq!(
            recordset(&result.rows()[3][1]).presence(),
            RecordsetPresence::Unassigned
        );
        assert!(recordset(&result.rows()[2][1]).is_empty());
        assert!(recordset(&result.rows()[3][1]).is_empty());
    }
    let count = run("SELECT Count(*) AS N FROM T", &data).unwrap();
    assert_eq!(
        count.rows(),
        [vec![QueryValue::Scalar(ExpressionValue::Value(
            Value::Int64(4)
        ))]]
    );
    assert!(count.recordset_schema(0).is_none());
    assert_eq!(data.requests.borrow().len(), 2);
    assert!(
        data.requests
            .borrow()
            .iter()
            .all(|request| request.table == "T"
                && request.scalar_indices == [0]
                && request.recordset_indices == [0])
    );
}

#[test]
fn aliases_share_owned_children_and_empty_results_keep_static_schemas() {
    let data = Data::new();
    let result = run(
        "SELECT Files AS PrimaryFiles,Id,Files AS Again FROM T WHERE Id=1",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 1);
    assert!(Arc::ptr_eq(
        recordset(&result.rows()[0][0]),
        recordset(&result.rows()[0][2])
    ));
    assert!(result.recordset_schema(0).is_some());
    assert!(result.recordset_schema(1).is_none());
    assert!(result.recordset_schema(2).is_some());
    let empty = run("SELECT Files AS Children FROM T WHERE Id<0", &data).unwrap();
    assert!(empty.rows().is_empty());
    assert_eq!(
        empty.recordset_schema(0).unwrap().columns()[1].name(),
        "FileData"
    );
}

#[test]
fn scalar_filters_sort_and_top_keep_parent_recordsets_aligned() {
    let data = Data::new();
    let result = run(
        "SELECT TOP 2 Id,Files FROM T WHERE Id>=2 ORDER BY Id DESC",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 2);
    for (row, index) in result.rows().iter().zip([3, 2]) {
        assert_eq!(row[0], scalar(index as i32 + 1));
        assert!(Arc::ptr_eq(recordset(&row[1]), &data.records[index]));
    }
}

#[test]
fn joins_keep_separate_origins_and_outer_absence_is_not_an_empty_parent() {
    let data = Data::new();
    let left = run("SELECT t.Id,t.Files AS L,u.Files AS R FROM T AS t LEFT JOIN U AS u ON t.Id=u.Id ORDER BY t.Id", &data).unwrap();
    assert_eq!(left.rows().len(), 4);
    assert_eq!(
        left.output_types()[2],
        OutputType::Recordset {
            parent_nullable: true
        }
    );
    for (index, row) in left.rows().iter().enumerate() {
        assert!(Arc::ptr_eq(recordset(&row[1]), &data.records[index]));
        if index == 1 {
            assert_eq!(
                recordset(&row[2]).rows()[0][0],
                StoredCell::Value(Value::Text("right-two".into()))
            );
        } else {
            assert_eq!(row[2], QueryValue::AbsentRecordset);
        }
    }
    let right = run("SELECT t.Files AS L,u.Id,u.Files AS R FROM T AS t RIGHT JOIN U AS u ON t.Id=u.Id ORDER BY u.Id", &data).unwrap();
    assert_eq!(right.rows().len(), 2);
    assert_eq!(right.rows()[1][0], QueryValue::AbsentRecordset);
    assert_eq!(right.rows()[1][1], scalar(5));
    assert_eq!(
        recordset(&right.rows()[1][2]).rows()[0][0],
        StoredCell::Value(Value::Text("right-five".into()))
    );
}

#[test]
fn derived_recordsets_keep_schema_and_written_inner_projections_remain_eager() {
    let data = Data::new();
    let result = run("SELECT d.Children,d.Id FROM (SELECT Id,Files AS Children FROM T) AS d WHERE d.Id>=3 ORDER BY d.Id", &data).unwrap();
    assert!(result.recordset_schema(0).is_some());
    for (row, index) in result.rows().iter().zip([2, 3]) {
        assert!(Arc::ptr_eq(recordset(&row[0]), &data.records[index]));
        assert_eq!(row[1], scalar(index as i32 + 1));
    }
    let before = data.requests.borrow().len();
    let scalar = run("SELECT d.Id FROM (SELECT Id,Files FROM T) AS d", &data).unwrap();
    assert_eq!(scalar.rows().len(), 4);
    assert_eq!(data.requests.borrow().len(), before + 1);
    assert_eq!(
        data.requests.borrow().last().unwrap().recordset_indices,
        [0]
    );
}

#[test]
fn structural_child_metadata_changes_refuse_before_any_row_scan() {
    for change in [1, 2, 3, 4, 6] {
        let data = Data::new();
        let parsed = parse_query("SELECT Files FROM T", Default::default()).unwrap();
        let prepared = prepare(parsed.query(), &data).unwrap();
        data.changed.set(change);
        assert!(matches!(
            prepared.execute_collections(&data, &[], ExecutionOptions::default()),
            Err(QueryError::SchemaChanged)
        ));
        assert_eq!(data.scans.get(), 0);
    }
    let data = Data::new();
    let parsed = parse_query("SELECT Files FROM T", Default::default()).unwrap();
    let prepared = prepare(parsed.query(), &data).unwrap();
    // A distinct immutable schema owner with identical declarations is valid;
    // source revalidation checks structure rather than Arc pointer identity.
    data.changed.set(5);
    assert_eq!(
        prepared
            .execute_collections(&data, &[], ExecutionOptions::default())
            .unwrap()
            .rows()
            .len(),
        4
    );
}

#[test]
fn requested_carrier_schema_width_and_failed_suffix_never_publish_partial_results() {
    for fault in 0..3 {
        let data = Data::new();
        match fault {
            0 => data.wrong_schema.set(true),
            1 => data.empty_callback.set(true),
            _ => data.fail_suffix.set(true),
        }
        assert!(matches!(
            run("SELECT Files FROM T", &data),
            Err(QueryError::Source)
        ));
        data.wrong_schema.set(false);
        data.empty_callback.set(false);
        data.fail_suffix.set(false);
        assert_eq!(run("SELECT Files FROM T", &data).unwrap().rows().len(), 4);
    }
}

#[test]
fn scalar_key_group_and_union_uses_refuse_before_scans_or_file_data_inspection() {
    let data = Data::new();
    for sql in [
        "SELECT Files.FileData FROM T",
        "SELECT Files+1 FROM T",
        "SELECT Files=Files FROM T",
        "SELECT DISTINCT Files FROM T",
        "SELECT Files FROM T GROUP BY Files",
        "SELECT Files FROM T ORDER BY Files",
        "SELECT Files FROM T UNION ALL SELECT Files FROM T",
    ] {
        assert!(run(sql, &data).is_err(), "{sql}");
        assert_eq!(data.scans.get(), 0, "{sql}");
    }
}

#[test]
fn byte_row_work_and_cancellation_limits_fail_without_partial_results() {
    let data = Data::new();
    let parsed = parse_query("SELECT Files FROM T", Default::default()).unwrap();
    let prepared = prepare(parsed.query(), &data).unwrap();
    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            prepared.execute_collections(
                &data,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(QueryError::Limit(_))
        ));
    }
    assert!(matches!(
        prepared.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| true),
                ..Default::default()
            }
        ),
        Err(QueryError::Cancelled)
    ));
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() > 40
    };
    assert!(matches!(
        prepared.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            }
        ),
        Err(QueryError::Cancelled)
    ));
    assert_eq!(
        prepared
            .execute_collections(&data, &[], ExecutionOptions::default())
            .unwrap()
            .rows()
            .len(),
        4
    );
}

#[test]
fn representational_recordset_equality_includes_kind_without_requiring_arc_identity() {
    let data = Data::new();
    let rows = || {
        vec![
            vec![
                StoredCell::Value(Value::Text("named.txt".into())),
                StoredCell::Uninspected,
            ],
            vec![StoredCell::Value(Value::Null), StoredCell::Uninspected],
        ]
    };
    let same = OwnedRecordset::new(
        Arc::clone(data.recordset_columns("T").unwrap()[0].schema()),
        rows(),
        RecordsetPresence::Assigned,
        &mut ResourceBudget::new(0, 0, 1000),
        &|| false,
    )
    .unwrap();
    let original = QueryValue::Recordset(Arc::clone(&data.records[0]));
    assert_eq!(original, QueryValue::Recordset(Arc::new(same)));
    data.changed.set(6);
    let changed = OwnedRecordset::new(
        Arc::clone(data.recordset_columns("T").unwrap()[0].schema()),
        rows(),
        RecordsetPresence::Assigned,
        &mut ResourceBudget::new(0, 0, 1000),
        &|| false,
    )
    .unwrap();
    assert_eq!(changed.schema().kind(), RecordsetKind::VersionHistory);
    assert_ne!(original, QueryValue::Recordset(Arc::new(changed)));
}
