#[path = "../../../engines/tablecraft-ace/tests/currency_indexes/fixture.rs"]
mod currency_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/scalar_fixture.rs"]
mod scalar_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, Source, prepare};
use tablecraft_sql::parse_query;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16384, 1_000_000)
}

#[test]
fn currency_seeks_fit_small_retention_limits_without_publishing_partial_table_caches() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let mut bytes = currency_fixture::database(
                code,
                descending,
                &[Some(-1), Some(1), Some(i64::MAX)],
                0,
            );
            if code == 1 {
                bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
            }
            let source = NativeSource::new_with_snapshot_limits(
                Cursor::new(bytes),
                CatalogScope::new(1),
                budget(),
                &|| false,
                TableSnapshotLimits::new(1, 1).unwrap(),
            )
            .unwrap()
            .with_index("Items", "BigKey")
            .unwrap();
            assert_eq!(source.columns("Items").unwrap().len(), 2);
            let query = parse_query(
                "PARAMETERS P CURRENCY; SELECT Id FROM Items WHERE Score=P",
                Default::default(),
            )
            .unwrap();
            let plan = prepare(query.query(), &source).unwrap();
            for (scaled, id) in [(1, 2), (i64::MAX, 3), (-1, 1)] {
                let result = plan
                    .execute(
                        &source,
                        &[("P", Value::Currency(Currency::from_scaled(scaled)))],
                        Default::default(),
                    )
                    .unwrap();
                assert_eq!(result.rows(), [vec![Value::Int32(id)]]);
            }
            let query = parse_query("SELECT Count(*) FROM Items", Default::default()).unwrap();
            assert_eq!(
                prepare(query.query(), &source)
                    .unwrap()
                    .execute(&source, &[], Default::default())
                    .err(),
                Some(QueryError::StructureLimit)
            );
        }
    }
}

#[test]
fn required_currency_index_corruption_never_falls_back_to_sequential_rows() {
    let mut bytes = currency_fixture::database(5, false, &[Some(1)], 0);
    // Authored layout: index root page 14; valid table data remains on page 12.
    bytes[14 * 4096] = 0;
    let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(1), budget(), &|| {
        false
    })
    .unwrap()
    .with_index("Items", "BigKey")
    .unwrap();
    let query = parse_query(
        "SELECT Id FROM Items WHERE Score=CCur(0.0001)",
        Default::default(),
    )
    .unwrap();
    assert!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .is_err()
    );
}
