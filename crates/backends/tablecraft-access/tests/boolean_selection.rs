#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, prepare};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const INDEX_PAGE: usize = 14;
const OWNER_OFFSET: usize = 4;

fn database(code: u8, descending: bool) -> Vec<u8> {
    let rows = [0, -1, 0, -1]
        .into_iter()
        .enumerate()
        .map(|(index, value)| vec![Some(index as i64 + 1), Some(value)])
        .collect::<Vec<_>>();
    let mut bytes = numeric_fixture::database(code, &[4, 1], &[(1, descending)], &rows, 0);
    if code == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

fn source(bytes: Vec<u8>, maximum_rows: u32) -> NativeSource<'static, Cursor<Vec<u8>>> {
    NativeSource::new_with_snapshot_limits(
        Cursor::new(bytes),
        CatalogScope::new(1),
        ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000),
        &|| false,
        TableSnapshotLimits::new(maximum_rows, 1).unwrap(),
    )
    .unwrap()
}

fn run(
    source: &NativeSource<'_, Cursor<Vec<u8>>>,
    sql: &str,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), source)?
        .execute(source, &[], Default::default())
        .map(|result| result.rows().to_vec())
}

#[test]
fn automatic_and_required_boolean_seeks_retain_table_order_and_never_complete_partial_caches() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            for automatic in [false, true] {
                let bytes = database(code, descending);
                let native = source(bytes, 2);
                let native = if automatic {
                    native.with_auto_indexes()
                } else {
                    native.with_index("Items", "Compound").unwrap()
                };
                for (literal, expected) in [("True", [2, 4]), ("False", [1, 3]), ("True", [2, 4])] {
                    let sql = format!("SELECT Id FROM Items WHERE Score={literal}");
                    assert_eq!(
                        run(&native, &sql).unwrap(),
                        expected.map(|id| vec![Value::Int32(id)])
                    );
                }
                assert_eq!(
                    run(&native, "SELECT Count(*) FROM Items").err(),
                    Some(QueryError::StructureLimit)
                );
            }
        }
    }
}

#[test]
fn selected_boolean_index_corruption_and_missing_required_index_never_fall_back() {
    let mut bytes = database(6, false);
    bytes[INDEX_PAGE * PAGE_BYTES + OWNER_OFFSET] = 0;
    for automatic in [false, true] {
        let native = source(bytes.clone(), 10);
        let native = if automatic {
            native.with_auto_indexes()
        } else {
            native.with_index("Items", "Compound").unwrap()
        };
        let query =
            parse_query("SELECT Id FROM Items WHERE Score=True", Default::default()).unwrap();
        assert!(prepare(query.query(), &native).is_ok());
        assert!(run(&native, "SELECT Id FROM Items WHERE Score=True").is_err());
    }
    let native = source(database(6, false), 10)
        .with_index("Items", "MissingIndex")
        .unwrap();
    assert!(run(&native, "SELECT Id FROM Items WHERE Score=True").is_err());
}

#[test]
fn boolean_auto_hints_decline_null_ranges_coercions_and_unsafe_conjunctions() {
    for filter in [
        "Score IS NULL",
        "Score>False",
        "Score=-1",
        "Score='True'",
        "Score=CBool(1)",
    ] {
        let native = source(database(6, false), 2).with_auto_indexes();
        let sql = format!("SELECT Id FROM Items WHERE {filter}");
        assert_eq!(run(&native, &sql).err(), Some(QueryError::StructureLimit));
    }
    let native = source(database(6, false), 10).with_auto_indexes();
    assert!(run(&native, "SELECT Id FROM Items WHERE Score=False AND 1/0=0").is_err());
}
