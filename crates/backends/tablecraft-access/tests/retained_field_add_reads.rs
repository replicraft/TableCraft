//! Independent retained nullable ADD keeps historical rows and complete siblings.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, inspect_table_properties, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields::{
    self as raw, Target,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn retained_number_and_text_add_preserve_old_records_as_null_without_default_backfill() {
    for case in raw::CASES {
        let source = raw::source(case);
        let scope = CatalogScope::new(3701);
        let original = read_table(
            &mut Cursor::new(&source.bytes),
            scope,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let sibling = read_table(
            &mut Cursor::new(&source.bytes),
            scope,
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for target in [Target::Number, Target::ShortText] {
            let after = raw::after(
                &source.bytes,
                source.definition,
                source.catalog_root,
                target,
            );
            let mut expected = original.rows().to_vec();
            for row in &mut expected {
                row.push(Value::Null);
            }
            let observed = read_table(
                &mut Cursor::new(&after),
                scope,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(observed.rows(), expected);
            assert_eq!(
                observed.schema().columns().last().unwrap().name(),
                target.name()
            );
            check_table(
                &mut Cursor::new(&after),
                scope,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                read_table(
                    &mut Cursor::new(&after),
                    scope,
                    "Other",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                sibling.rows()
            );
            inspect_table_properties(
                &mut Cursor::new(&after),
                scope,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let data = (source.definition as usize + 1) * 4096;
            assert_eq!(
                &after[data..data + 2 * 4096],
                &source.bytes[data..data + 2 * 4096]
            );
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("added.accdb");
            fs::write(&path, &after).unwrap();
            let fresh = NativeSource::new(
                fs::File::open(&path).unwrap(),
                CatalogScope::new(3702),
                budget(),
                &|| false,
            )
            .unwrap();
            let query =
                parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default()).unwrap();
            assert_eq!(
                prepare(query.query(), &fresh)
                    .unwrap()
                    .execute(&fresh, &[], ExecutionOptions::default())
                    .unwrap()
                    .rows(),
                expected
            );
            assert_eq!(fs::read(path).unwrap(), after);
        }
    }
}
