use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

const PAGE_BYTES: usize = 4096;
const OWNER_PAGE: usize = 10;
const FIRST_FIELD: usize = 63;
const FIELD_BYTES: usize = 25;
const UNKNOWN_TYPE: u8 = 0xff;

#[test]
fn native_queries_preserve_unavailable_names_without_null_placeholders() {
    for opaque_only in [false, true] {
        let mut bytes = fixture::database();
        bytes[OWNER_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES] = UNKNOWN_TYPE;
        if opaque_only {
            bytes[OWNER_PAGE * PAGE_BYTES + FIRST_FIELD] = UNKNOWN_TYPE;
        }
        let original = bytes.clone();
        let source = NativeSource::new(
            Cursor::new(&bytes),
            CatalogScope::new(1),
            ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
            &|| false,
        )
        .unwrap();
        let unavailable = source.unavailable_columns("Items").unwrap();
        assert_eq!(
            unavailable
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            if opaque_only {
                vec!["Id", "Score"]
            } else {
                vec!["Score"]
            }
        );
        assert_eq!(
            source.columns("Items").unwrap().len(),
            usize::from(!opaque_only)
        );
        for sql in [
            "SELECT * FROM Items",
            "SELECT Score FROM Items",
            "SELECT Id FROM Items WHERE Score IS NULL",
        ] {
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            assert_eq!(
                prepare(parsed.query(), &source).err(),
                Some(QueryError::Unsupported)
            );
        }
        let parsed = parse_query("SELECT Count(*) FROM Items", ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int64(3)]]
        );
        if !opaque_only {
            let parsed = parse_query(
                "SELECT Id FROM Items WHERE Id >= 2 ORDER BY Id DESC",
                ParseLimits::default(),
            )
            .unwrap();
            assert_eq!(
                prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(&source, &[], ExecutionOptions::default())
                    .unwrap()
                    .rows(),
                &[vec![Value::Int32(3)], vec![Value::Int32(2)]]
            );
        }
        assert_eq!(bytes, original);
    }
}

#[path = "../../../engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
mod index_fixture;
#[test]
fn indexed_mixed_scalar_views_translate_dense_constraint_positions() {
    for descending in [false, true] {
        let mut bytes = index_fixture::database(descending, false, true);
        const INDEXED_FIELD_START: usize = 75;
        bytes[OWNER_PAGE * PAGE_BYTES + INDEXED_FIELD_START] = UNKNOWN_TYPE;
        let source = NativeSource::new(
            Cursor::new(&bytes),
            CatalogScope::new(1),
            ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
            &|| false,
        )
        .unwrap()
        .with_index("Items", "ScoreKey")
        .unwrap();
        assert_eq!(source.columns("Items").unwrap()[0].name(), "Score");
        for (sql, expected) in [
            (
                "SELECT Score FROM Items WHERE Score=20",
                vec![vec![Value::Int32(20)]],
            ),
            (
                "SELECT Score FROM Items WHERE Score IS NULL",
                vec![vec![Value::Null]],
            ),
            (
                "SELECT Score FROM Items WHERE Score>=20 ORDER BY Score",
                vec![vec![Value::Int32(20)], vec![Value::Int32(30)]],
            ),
            ("SELECT Count(*) FROM Items", vec![vec![Value::Int64(3)]]),
        ] {
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            assert_eq!(
                prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(&source, &[], ExecutionOptions::default())
                    .unwrap()
                    .rows(),
                expected
            );
        }
    }
}

#[test]
fn mixed_view_range_pushdown_skips_a_disjoint_corrupt_index_leaf() {
    let mut bytes = index_fixture::node_database();
    const INDEXED_FIELD_START: usize = 75;
    const DISJOINT_LEAF: usize = 15;
    bytes[OWNER_PAGE * PAGE_BYTES + INDEXED_FIELD_START] = UNKNOWN_TYPE;
    bytes[DISJOINT_LEAF * PAGE_BYTES] = 0;
    let source = NativeSource::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
    .unwrap()
    .with_index("Items", "ScoreKey")
    .unwrap();
    let parsed = parse_query(
        "SELECT Score FROM Items WHERE Score=30",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int32(30)]]
    );
    let parsed = parse_query("SELECT Score FROM Items", ParseLimits::default()).unwrap();
    assert!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .is_err()
    );
}
