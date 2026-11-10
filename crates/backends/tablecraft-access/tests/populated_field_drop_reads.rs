//! Independent tail-DROP images stay readable/queryable through fresh file hosts.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{inspect_table_schema, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated as raw;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1421);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn populated_tail_drop_keeps_raw_storage_and_literal_survivors_readable() {
    for case in raw::CASES {
        let [before, after] = raw::pair(case);
        let expected = raw::expected_rows(case);
        assert_eq!(&after[8 * PAGE..10 * PAGE], &before[8 * PAGE..10 * PAGE]);
        assert_eq!(
            &after[7 * PAGE + 41..7 * PAGE + 45],
            &before[7 * PAGE + 41..7 * PAGE + 45]
        );
        let original = inspect_table_schema(
            &mut Cursor::new(&before),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let current = inspect_table_schema(
            &mut Cursor::new(&after),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(current.declared_rows(), original.declared_rows());
        assert_eq!(current.columns().len() + 1, original.columns().len());
        assert!(
            !current
                .columns()
                .iter()
                .any(|field| field.name() == raw::column(case))
        );
        for field in current.columns() {
            let old = original
                .columns()
                .iter()
                .find(|old| old.number() == field.number())
                .unwrap();
            assert_eq!(field.name(), old.name());
            assert_eq!(field.native_type(), old.native_type());
            assert_eq!(field.flags(), old.flags());
            assert_eq!(field.extended_flags(), old.extended_flags());
            assert_eq!(field.storage_length(), old.storage_length());
            assert_eq!(field.text_sort_order(), old.text_sort_order());
        }
        let table = read_table(
            &mut Cursor::new(&after),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap_or_else(|error| panic!("{case:?} after read: {error:?}"));
        assert_eq!(table.rows(), expected, "{case:?}");
    }
}

#[test]
fn fresh_file_queries_use_surviving_schema_and_refuse_deleted_names() {
    for case in raw::CASES {
        let [_, after] = raw::pair(case);
        let expected = raw::expected_rows(case);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("after.accdb");
        fs::write(&path, &after).unwrap();
        let source =
            NativeSource::new(fs::File::open(&path).unwrap(), SCOPE, budget(), &|| false).unwrap();
        let sql = parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default()).unwrap();
        let rows = prepare(sql.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(rows.rows(), expected, "{case:?}");
        let count = parse_query("SELECT Count(*) FROM Table1", Default::default()).unwrap();
        assert_eq!(
            prepare(count.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int64(expected.len() as i64)]]
        );
        let removed = parse_query(
            &format!("SELECT [{}] FROM Table1", raw::column(case)),
            Default::default(),
        )
        .unwrap();
        assert!(
            prepare(removed.query(), &source).is_err(),
            "deleted field still binds: {case:?}"
        );
        drop(source);
        assert_eq!(fs::read(&path).unwrap(), after);
        let reopened = NativeSource::new(
            fs::File::open(&path).unwrap(),
            CatalogScope::new(1422),
            budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            prepare(sql.query(), &reopened)
                .unwrap()
                .execute(&reopened, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            expected
        );
        drop(reopened);
        assert_eq!(fs::read(&path).unwrap(), after);
    }
}
