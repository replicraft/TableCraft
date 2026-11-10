//! Independent retained nullable ADD keeps historical rows and complete siblings.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, inspect_table_properties, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields::{self as raw, Target, property_pages},
    rows::Input,
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

#[test]
fn retained_added_fields_materialize_old_rows_then_insert_literal_defaults_null_and_values() {
    for case in raw::CASES {
        let source = raw::source(case);
        for target in [Target::Number, Target::ShortText] {
            let added = raw::after(
                &source.bytes,
                source.definition,
                source.catalog_root,
                target,
            );
            let replacement = match target {
                Target::Number => Value::Int32(123),
                Target::ShortText => Value::Text("日本語🦀".into()),
            };
            let filled = raw::fill(
                &added,
                source.definition,
                source.roots[0],
                target,
                Some(replacement.clone()),
            );
            let observed = read_table(
                &mut Cursor::new(&filled),
                CatalogScope::new(3703),
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(observed.rows()[0].last(), Some(&replacement));
            assert!(
                observed.rows()[1..]
                    .iter()
                    .all(|row| row.last() == Some(&Value::Null))
            );
            let mut before = filled;
            for input in [
                Input::Omitted,
                Input::Null,
                match target {
                    Target::Number => Input::Number(-7),
                    Target::ShortText => Input::Text(""),
                },
            ] {
                let inserted = raw::append(&before, source.definition, target, input);
                let observed = read_table(
                    &mut Cursor::new(&inserted),
                    CatalogScope::new(3704),
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let expected = match (target, input) {
                    (Target::Number, Input::Omitted) => Value::Int32(0),
                    (_, Input::Omitted | Input::Null) => Value::Null,
                    (Target::Number, Input::Number(value)) => Value::Int32(value),
                    (Target::ShortText, Input::Text(value)) => Value::Text(value.into()),
                    _ => unreachable!(),
                };
                assert_eq!(observed.rows().last().unwrap().last(), Some(&expected));
                assert_eq!(observed.rows()[0].last(), Some(&replacement));
                check_table(
                    &mut Cursor::new(&inserted),
                    CatalogScope::new(3705),
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                before = inserted;
            }
        }
    }
}

#[test]
fn retained_property_add_uses_the_other_actual_owned_hinted_lval_page_when_one_is_full() {
    for case in property_pages::CASES {
        let source = property_pages::source(case);
        let original = read_table(
            &mut Cursor::new(&source.bytes),
            CatalogScope::new(3706),
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for target in [Target::Number, Target::ShortText] {
            let after = property_pages::after(&source, target);
            let observed = read_table(
                &mut Cursor::new(&after),
                CatalogScope::new(3707),
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let mut expected = original.rows().to_vec();
            for row in &mut expected {
                row.push(Value::Null);
            }
            assert_eq!(observed.rows(), expected);
            inspect_table_properties(
                &mut Cursor::new(&after),
                CatalogScope::new(3708),
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            check_table(
                &mut Cursor::new(&after),
                CatalogScope::new(3709),
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let unchanged = source.other_page as usize * 4096;
            assert_eq!(
                &after[unchanged..unchanged + 4096],
                &source.bytes[unchanged..unchanged + 4096]
            );
            assert_eq!(after[source.owned_cell], source.bytes[source.owned_cell]);
            assert_eq!(after[source.hint_cell], source.bytes[source.hint_cell]);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("property-pages.accdb");
            fs::write(&path, &after).unwrap();
            let fresh = NativeSource::new(
                fs::File::open(&path).unwrap(),
                CatalogScope::new(3710),
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
