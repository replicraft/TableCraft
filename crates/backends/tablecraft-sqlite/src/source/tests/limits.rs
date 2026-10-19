use super::*;
use std::sync::atomic::Ordering;
use tablecraft_core::limits::Resource;

#[test]
fn cancellation_is_shared_and_teardown_remains_available() {
    let cancelled = Arc::new(AtomicBool::new(false));
    // Use an explicitly owned cancellation flag, as public callers do.
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE TABLE Items(ID INTEGER); INSERT INTO Items VALUES(1),(2)")
        .unwrap();
    let controlled =
        SqliteSource::initialize(connection, Work::new(budget(), cancelled.clone())).unwrap();
    cancelled.store(true, Ordering::Relaxed);
    assert_eq!(rows(&controlled, "Items"), Err(QueryError::Cancelled));
    controlled.close().unwrap();
}

#[test]
fn bytes_rows_and_vm_steps_are_cumulative_and_typed() {
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let source = source("CREATE TABLE Items(ID INTEGER); INSERT INTO Items VALUES(1),(2);");
        assert!(source.columns("Items").is_ok());
        // Exhaust one dimension after metadata without resetting the others.
        let remaining = source.work.state_for_test(resource);
        source.work.charge(resource, remaining).unwrap();
        assert!(
            matches!(rows(&source, "Items"), Err(QueryError::Limit(error)) if error.resource == resource)
        );
        source.close().unwrap();
    }
}

#[test]
fn sqlite_vm_work_can_interrupt_before_returning_a_record() {
    let source = source("CREATE TABLE Items(ID INTEGER); INSERT INTO Items VALUES(1);");
    let remaining = source.work.state_for_test(Resource::Steps);
    source.work.charge(Resource::Steps, remaining).unwrap();
    let error = source.work.sql(source.connection.query_row(
        "WITH RECURSIVE N(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM N WHERE x<1000000) SELECT sum(x) FROM N",
        [], |row| row.get::<_, i64>(0),
    ));
    assert!(matches!(error, Err(SqliteError::Limit(error)) if error.resource == Resource::Steps));
    source.close().unwrap();
}

#[test]
fn structural_limits_reject_oversized_names_values_and_table_inventories() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(&format!("CREATE TABLE \"{}\"(ID INTEGER)", "X".repeat(256)))
        .unwrap();
    assert!(matches!(
        SqliteSource::initialize(
            connection,
            Work::new(budget(), Arc::new(AtomicBool::new(false)))
        ),
        Err(SqliteError::Schema(_))
    ));

    let connection = Connection::open_in_memory().unwrap();
    for index in 0..257 {
        connection
            .execute_batch(&format!("CREATE TABLE T{index}(ID INTEGER)"))
            .unwrap();
    }
    assert!(matches!(
        SqliteSource::initialize(
            connection,
            Work::new(budget(), Arc::new(AtomicBool::new(false)))
        ),
        Err(SqliteError::StructureLimit)
    ));

    let source = source("CREATE TABLE Items(B BLOB); INSERT INTO Items VALUES(zeroblob(1048577));");
    assert_eq!(rows(&source, "Items"), Err(QueryError::StructureLimit));
}
