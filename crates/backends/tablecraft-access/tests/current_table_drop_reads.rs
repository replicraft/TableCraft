//! Independent logical table DROP retains storage while removing public ownership.
use std::{collections::BTreeSet, fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{inspect_catalog, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_delete,
        deleted_field::populated::{CASES, readdition::Target},
    },
    canonical_table_drop::current as raw,
};

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 40_000_000)
}

fn names(bytes: &[u8]) -> BTreeSet<String> {
    inspect_catalog(
        &mut Cursor::new(bytes),
        CatalogScope::new(1821),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .catalog()
    .objects()
    .iter()
    .map(|object| object.name().as_str().to_owned())
    .collect()
}

#[test]
fn current_table_drop_removes_only_logical_rows_index_membership_and_catalog_owner() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for history in raw::HISTORIES {
                    if matches!(history, raw::History::Initial)
                        && (!matches!(target, Target::Number) || filled)
                    {
                        continue;
                    }
                    let source = raw::source(case, target, filled, history);
                    let after = raw::after(&source);
                    let mut expected_names = names(&source.bytes);
                    assert!(expected_names.remove("Table1"));
                    assert_eq!(names(&after), expected_names);
                    assert_eq!(after.len(), source.bytes.len());
                    assert_eq!(&after[..2 * PAGE], &source.bytes[..2 * PAGE]);
                    assert_eq!(
                        &after[3 * PAGE..4 * PAGE],
                        &source.bytes[3 * PAGE..4 * PAGE]
                    );
                    assert_eq!(
                        &after[5 * PAGE..7 * PAGE],
                        &source.bytes[5 * PAGE..7 * PAGE]
                    );
                    assert_eq!(&after[10 * PAGE..], &source.bytes[10 * PAGE..]);
                    let row_deleted = current_delete::after(&source.bytes, &source.roots);
                    let differences = row_deleted
                        .iter()
                        .zip(&after)
                        .enumerate()
                        .filter_map(|(offset, (old, new))| (old != new).then_some(offset))
                        .collect::<Vec<_>>();
                    assert_eq!(differences, [2 * PAGE + 16, 4 * PAGE + 14 + 2 * 7 + 1]);
                    assert!(
                        read_table(
                            &mut Cursor::new(&after),
                            CatalogScope::new(1822),
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .is_err(),
                        "{case:?}/{target:?}/{filled}/{history:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn fresh_query_hosts_cannot_bind_dropped_table_but_keep_database_property_catalog_owner() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for history in raw::HISTORIES {
                if matches!(history, raw::History::Initial) && !matches!(target, Target::Number) {
                    continue;
                }
                let original = raw::source(case, target, true, history);
                let after = raw::after(&original);
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("dropped.accdb");
                fs::write(&path, &after).unwrap();
                for scope in [1823, 1824] {
                    let source = NativeSource::new(
                        fs::File::open(&path).unwrap(),
                        CatalogScope::new(scope),
                        budget(),
                        &|| false,
                    )
                    .unwrap();
                    let dropped = parse_query("SELECT * FROM Table1", Default::default()).unwrap();
                    assert!(prepare(dropped.query(), &source).is_err());
                    let query = parse_query(
                        "SELECT Name FROM MSysObjects WHERE Name = 'MSysDb'",
                        Default::default(),
                    )
                    .unwrap();
                    let result = prepare(query.query(), &source)
                        .unwrap()
                        .execute(&source, &[], ExecutionOptions::default())
                        .unwrap();
                    assert_eq!(result.rows(), &[vec![Value::Text("MSysDb".into())]]);
                }
                assert_eq!(fs::read(path).unwrap(), after);
            }
        }
    }
}
