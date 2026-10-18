mod files;
mod limits;

use super::*;
use proptest::prelude::*;
use tablecraft_core::values::Double;
use tablecraft_query::{ExecutionOptions, prepare};

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

fn source(sql: &str) -> SqliteSource {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(sql).unwrap();
    SqliteSource::initialize(
        connection,
        Work::new(budget(), Arc::new(AtomicBool::new(false))),
    )
    .unwrap()
}

fn rows(source: &SqliteSource, table: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let mut rows = Vec::new();
    source.scan(table, &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    })?;
    Ok(rows)
}

#[test]
fn exact_carriers_null_empty_unicode_and_extremes_are_preserved() {
    let source = source(
        "CREATE TABLE Scalars (I INTEGER, R REAL, T TEXT, B BLOB);
        INSERT INTO Scalars VALUES(-9223372036854775808,1.5,'é🦀',X'00FF');
        INSERT INTO Scalars VALUES(9223372036854775807,2,'',X'');
        INSERT INTO Scalars VALUES(NULL,NULL,NULL,NULL);",
    );
    assert_eq!(
        rows(&source, "scalars").unwrap(),
        vec![
            vec![
                Value::Int64(i64::MIN),
                Value::Double(Double::from_float(1.5)),
                Value::Text("é🦀".into()),
                Value::Binary(vec![0, 255])
            ],
            vec![
                Value::Int64(i64::MAX),
                Value::Double(Double::from_float(2.0)),
                Value::Text(String::new()),
                Value::Binary(Vec::new())
            ],
            vec![Value::Null; 4],
        ]
    );
    assert!(
        source
            .connection
            .execute("DELETE FROM Scalars", [])
            .is_err()
    );
    source.close().unwrap();
}

#[test]
fn access_query_semantics_run_over_sqlite_rows_without_sql_translation() {
    let source = source(
        "CREATE TABLE Items(ID INTEGER, Label TEXT);
        INSERT INTO Items VALUES(1,'a'),(2,'b'),(3,'c');",
    );
    let parsed = tablecraft_sql::parse_query(
        "SELECT TOP 2 ID, Label & '!' AS Caption FROM Items WHERE ID >= 2 ORDER BY ID DESC",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        &[
            vec![Value::Int64(3), Value::Text("c!".into())],
            vec![Value::Int64(2), Value::Text("b!".into())],
        ]
    );
}

#[test]
fn metadata_discovery_does_not_scan_or_coerce_mixed_runtime_values() {
    let source = source(
        "CREATE TABLE Mixed(ID INTEGER); INSERT INTO Mixed VALUES('private payload');
        CREATE TABLE Unknown(D DECIMAL); CREATE TABLE NoType(X);
        CREATE TABLE Dates(D DATETIME); CREATE TABLE Flexible(A ANY) STRICT;",
    );
    assert!(source.columns("Mixed").is_ok());
    assert_eq!(rows(&source, "Mixed"), Err(QueryError::Source));
    for table in ["Unknown", "NoType", "Dates", "Flexible"] {
        assert!(matches!(
            source.columns(table),
            Err(QueryError::Unsupported)
        ));
    }
    assert!(!format!("{:?}", SqliteError::Value).contains("private payload"));
}

#[test]
fn unsupported_executable_objects_are_not_queried() {
    let source = source(
        "CREATE TABLE Ordinary(ID INTEGER);
        CREATE VIEW Dangerous AS SELECT load_extension('not executed');
        CREATE TABLE Generated(ID INTEGER, G INTEGER GENERATED ALWAYS AS (abs(ID)) STORED);
        CREATE VIRTUAL TABLE Search USING fts5(body);",
    );
    assert!(source.columns("Ordinary").is_ok());
    assert!(matches!(
        source.columns("Dangerous"),
        Err(QueryError::UnknownName)
    ));
    assert!(matches!(
        source.columns("Generated"),
        Err(QueryError::Unsupported)
    ));
    assert!(matches!(
        source.columns("Search"),
        Err(QueryError::UnknownName)
    ));
    assert!(matches!(
        source.columns("Search_data"),
        Err(QueryError::UnknownName)
    ));
}

#[test]
fn visitor_failure_stops_immediately_and_is_not_reclassified() {
    let source = source("CREATE TABLE Items(ID INTEGER); INSERT INTO Items VALUES(1),(2);");
    let mut visited = 0;
    let error = source.scan("Items", &mut |_| {
        visited += 1;
        Err(QueryError::Parameter)
    });
    assert_eq!(error, Err(QueryError::Parameter));
    assert_eq!(visited, 1);
}

#[test]
fn malformed_utf8_and_primary_key_nullability_are_explicit() {
    let source = source(
        "CREATE TABLE Texts(T TEXT); INSERT INTO Texts VALUES(CAST(X'FF' AS TEXT));
        CREATE TABLE Keys(K TEXT PRIMARY KEY); INSERT INTO Keys VALUES(NULL);",
    );
    assert_eq!(rows(&source, "Texts"), Err(QueryError::Source));
    assert!(source.columns("Keys").unwrap()[0].nullable());
    assert_eq!(rows(&source, "Keys").unwrap(), vec![vec![Value::Null]]);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x53514c495445),
        ..Default::default()
    })]
    #[test]
    fn identifiers_are_quoted_and_text_blob_values_round_trip(
        suffix in "[a-zA-Z0-9\" ;_-]{0,30}",
        value in any::<String>(),
        bytes in prop::collection::vec(any::<u8>(), 0..100),
        integer in any::<i64>(),
    ) {
        let table = format!("T{suffix}");
        let column = format!("C{suffix}");
        let quote = |name: &str| format!("\"{}\"", name.replace('"', "\"\""));
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(&format!("CREATE TABLE {} ({} TEXT, B BLOB, I INTEGER)", quote(&table), quote(&column))).unwrap();
        connection.execute(&format!("INSERT INTO {} VALUES(?1,?2,?3)", quote(&table)), rusqlite::params![value, bytes, integer]).unwrap();
        let source = SqliteSource::initialize(connection, Work::new(budget(), Arc::new(AtomicBool::new(false)))).unwrap();
        prop_assert_eq!(rows(&source, &table).unwrap(), vec![vec![Value::Text(value), Value::Binary(bytes), Value::Int64(integer)]]);
    }
}
