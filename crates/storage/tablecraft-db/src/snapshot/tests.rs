use super::*;
use crate::SnapshotError;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::{Column, NameComparison, ScalarType, SchemaPolicy};

fn policy() -> SchemaPolicy {
    SchemaPolicy {
        max_name_bytes: 64,
        max_columns: 4,
        names: NameComparison::AsciiInsensitive,
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(100_000, 100, 100_000)
}
fn table() -> Snapshot {
    Snapshot::new(
        "Items".into(),
        vec![Column::new("Score".into(), ScalarType::Int32, true).unwrap()],
        vec![vec![Value::Int32(7)], vec![Value::Null]],
        policy(),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn row_handles_are_bound_to_one_exact_snapshot_not_displayed_values() {
    let first = table();
    let second = table();
    let row = first.row_handle(0).unwrap();
    assert_eq!(first.resolve(row).unwrap(), &[Value::Int32(7)]);
    assert_eq!(second.resolve(row), Err(SnapshotError::StaleRow));
    assert_ne!(row, second.row_handle(0).unwrap());
    assert!(first.row_handle(2).is_none());
    assert_eq!(
        first.resolve(first.row_handle(1).unwrap()).unwrap(),
        &[Value::Null]
    );
}

#[test]
fn exact_carriers_schema_policy_cancellation_and_work_are_checked() {
    let columns = vec![Column::new("Score".into(), ScalarType::Int32, false).unwrap()];
    assert!(matches!(
        Snapshot::new(
            "Items".into(),
            columns.clone(),
            vec![vec![Value::Int64(7)]],
            policy(),
            &mut budget(),
            &|| false
        ),
        Err(SnapshotError::Schema(_))
    ));
    assert_eq!(
        Snapshot::new(
            "Items".into(),
            columns.clone(),
            vec![],
            policy(),
            &mut budget(),
            &|| true
        )
        .err(),
        Some(SnapshotError::Cancelled)
    );
    assert!(matches!(
        Snapshot::new(
            "Items".into(),
            columns,
            vec![],
            policy(),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(SnapshotError::Limit(_))
    ));
}
