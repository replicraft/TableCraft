//! Fresh read/index/query witnesses span retained DDL and DML over long roots.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    cross_operation, deletes,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn observe(bytes: &[u8], expected: &[Vec<Value>], sibling: bool, final_sibling: bool) {
    let scope = CatalogScope::new(3501);
    for (table, rows) in [("Table1", expected), ("Other", &[][..]), ("More", &[][..])] {
        if (table == "Other" && !sibling) || (table == "More" && !final_sibling) {
            continue;
        }
        assert_eq!(
            read_table(
                &mut Cursor::new(bytes),
                scope,
                table,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            rows
        );
        check_table(
            &mut Cursor::new(bytes),
            scope,
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cross-operation.accdb");
    fs::write(&path, bytes).unwrap();
    let source = NativeSource::new(
        fs::File::open(&path).unwrap(),
        CatalogScope::new(3502),
        budget(),
        &|| false,
    )
    .unwrap();
    let query = parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        expected
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn long_catalog_and_user_roots_survive_append_insert_update_delete_and_reinsert() {
    let stages = cross_operation::source();
    let initial = vec![vec![Value::Int32(1), Value::Int32(17)]];
    let inserted = vec![initial[0].to_vec(), vec![Value::Int32(2), Value::Int32(0)]];
    let updated = vec![
        vec![Value::Int32(1), Value::Int32(123)],
        inserted[1].to_vec(),
    ];
    let reinserted = vec![vec![Value::Int32(3), Value::Int32(0)]];
    for (bytes, rows, sibling, more) in [
        (&stages.initial, initial.as_slice(), false, false),
        (&stages.appended, initial.as_slice(), true, false),
        (&stages.inserted, inserted.as_slice(), true, false),
        (&stages.updated, updated.as_slice(), true, false),
        (&stages.first_deleted, &inserted[1..], true, false),
        (&stages.empty, &[][..], true, false),
        (&stages.reinserted, reinserted.as_slice(), true, false),
        (&stages.appended_again, reinserted.as_slice(), true, true),
    ] {
        assert_eq!(
            &bytes[7 * 4096..11 * 4096],
            &stages.initial[7 * 4096..11 * 4096]
        );
        observe(bytes, rows, sibling, more);
    }
    assert_eq!(stages.inserted_root, 21);
    assert_eq!(
        deletes::record(&stages.updated, 11, 0),
        &[2, 0, 1, 0, 0, 0, 123, 0, 0, 0, 3]
    );
    assert_eq!(
        &stages.appended[14 * 4096..17 * 4096],
        &stages.reinserted[14 * 4096..17 * 4096]
    );
    assert_eq!(
        &stages.reinserted[11 * 4096..17 * 4096],
        &stages.appended_again[11 * 4096..17 * 4096]
    );
}
