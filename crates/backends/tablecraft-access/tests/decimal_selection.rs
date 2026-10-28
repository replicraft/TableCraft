#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[path = "../../../engines/tablecraft-ace/tests/decimal_indexes/fixture.rs"]
mod decimal_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Double, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, prepare};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const INDEX_PAGE: usize = 14;
const OWNER_OFFSET: usize = 4;
const MAXIMUM: i128 = 9_999_999_999_999_999_999_999_999_999;

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
    parameters: &[(&str, Value)],
) -> Result<Vec<Vec<Value>>, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), source)?
        .execute(source, parameters, Default::default())
        .map(|result| result.rows().to_vec())
}

fn point(
    source: &NativeSource<'_, Cursor<Vec<u8>>>,
    value: Decimal,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let sql = format!(
        "PARAMETERS P Decimal(28,{}); SELECT Id FROM Items WHERE Score=P",
        value.scale(),
    );
    run(source, &sql, &[("P", Value::Decimal(value))])
}

#[test]
fn decimal_points_rescale_losslessly_and_never_complete_partial_caches() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            for automatic in [false, true] {
                let bytes = decimal_fixture::database_for(
                    code,
                    descending,
                    28,
                    4,
                    &[
                        None,
                        Some(-10000),
                        Some(0),
                        Some(10000),
                        Some(0),
                        Some(MAXIMUM),
                    ],
                    0,
                );
                let native = source(bytes, 2);
                let native = if automatic {
                    native.with_auto_indexes()
                } else {
                    native.with_index("Items", "Compound").unwrap()
                };
                for (coefficient, scale, ids) in [
                    (1, 0, vec![4]),
                    (100000, 5, vec![4]),
                    (-1, 0, vec![2]),
                    (0, 28, vec![3, 5]),
                    (MAXIMUM, 4, vec![6]),
                ] {
                    let expected = ids
                        .into_iter()
                        .map(|id| vec![Value::Int32(id)])
                        .collect::<Vec<_>>();
                    assert_eq!(
                        point(&native, Decimal::new(coefficient, scale).unwrap()).unwrap(),
                        expected
                    );
                }
                assert_eq!(
                    run(&native, "SELECT Id FROM Items WHERE Score IS NULL", &[]).unwrap(),
                    [vec![Value::Int32(1)]],
                );
                assert_eq!(
                    run(&native, "SELECT Count(*) FROM Items", &[]).err(),
                    Some(QueryError::StructureLimit),
                );
                assert_eq!(
                    point(&native, Decimal::new(0, 0).unwrap()).unwrap(),
                    [vec![Value::Int32(3)], vec![Value::Int32(5)]],
                );
            }
        }
    }
}

#[test]
fn lossy_or_out_of_precision_points_and_non_decimal_forms_decline_seeks() {
    for automatic in [false, true] {
        let bytes = decimal_fixture::database(false, 4, 2, &[None, Some(100), Some(200)], 0);
        let native = source(bytes, 1);
        let native = if automatic {
            native.with_auto_indexes()
        } else {
            native.with_index("Items", "Compound").unwrap()
        };
        for value in [
            Decimal::new(10001, 4).unwrap(),
            Decimal::new(10000, 2).unwrap(),
        ] {
            assert_eq!(
                point(&native, value).err(),
                Some(QueryError::StructureLimit)
            );
        }
        for sql in [
            "SELECT Id FROM Items WHERE Score=CDec(1)",
            "SELECT Id FROM Items WHERE Score=1",
            "SELECT Id FROM Items WHERE Score IS NOT NULL",
        ] {
            assert_eq!(
                run(&native, sql, &[]).err(),
                Some(QueryError::StructureLimit)
            );
        }
        assert_eq!(
            run(
                &native,
                "PARAMETERS P Double; SELECT Id FROM Items WHERE Score=P",
                &[("P", Value::Double(Double::from_float(1.0)))],
            )
            .err(),
            Some(QueryError::StructureLimit),
        );
    }
}

#[test]
fn chosen_decimal_index_failures_and_unsafe_eager_predicates_never_fall_back() {
    for automatic in [false, true] {
        let mut bytes = decimal_fixture::database(false, 28, 4, &[Some(10000), None], 0);
        bytes[INDEX_PAGE * PAGE_BYTES + OWNER_OFFSET] = 0;
        let native = source(bytes, 10);
        let native = if automatic {
            native.with_auto_indexes()
        } else {
            native.with_index("Items", "Compound").unwrap()
        };
        let query = parse_query(
            "PARAMETERS P Decimal(28,0); SELECT Id FROM Items WHERE Score=P",
            Default::default(),
        )
        .unwrap();
        assert!(prepare(query.query(), &native).is_ok());
        assert!(point(&native, Decimal::new(1, 0).unwrap()).is_err());
    }

    let bytes = decimal_fixture::database(false, 28, 4, &[Some(10000), None], 0);
    let native = source(bytes.clone(), 10).with_auto_indexes();
    assert!(
        run(
            &native,
            "PARAMETERS P Decimal(28,0); SELECT Id FROM Items WHERE Score=P And 1/0=0",
            &[("P", Value::Decimal(Decimal::new(1, 0).unwrap()))],
        )
        .is_err()
    );

    let native = source(bytes, 10)
        .with_index("Items", "MissingIndex")
        .unwrap();
    assert!(point(&native, Decimal::new(1, 0).unwrap()).is_err());
}
