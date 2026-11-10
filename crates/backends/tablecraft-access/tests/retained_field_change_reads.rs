//! Independent retained field names, tail removal and readdition keep raw values.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, inspect_table_properties, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes as changes,
    fields::{self, Case, Target},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn rows(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(3801),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

fn observe(bytes: &[u8], expected: &[Vec<Value>]) {
    assert_eq!(rows(bytes), expected);
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(3802),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    inspect_table_properties(
        &mut Cursor::new(bytes),
        CatalogScope::new(3803),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("field-changes.accdb");
    fs::write(&path, bytes).unwrap();
    let fresh = NativeSource::new(
        fs::File::open(&path).unwrap(),
        CatalogScope::new(3804),
        budget(),
        &|| false,
    )
    .unwrap();
    let query = parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &fresh)
            .unwrap()
            .execute(&fresh, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        expected
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn direct_initial_inline_property_rename_does_not_require_a_prior_add() {
    for case in fields::CASES {
        let source = fields::source(case);
        let old = match case {
            Case::FirstNumber => "Score",
            Case::SecondText => "Name",
            Case::OriginalSparse => "AfterLabel",
        };
        let after = changes::rename(
            &source.bytes,
            source.definition,
            source.catalog_root,
            old,
            "InitialRenamed",
        );
        observe(&after, &rows(&source.bytes));
        let data = (source.definition as usize + 1) * 4096;
        assert_eq!(
            &after[data..data + 2 * 4096],
            &source.bytes[data..data + 2 * 4096]
        );
    }
}

#[test]
fn retained_rename_drop_readd_keeps_highwater_ids_raw_payloads_and_new_null_presence() {
    for case in fields::CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                let source = changes::source(case, target, filled, true, true);
                let expected = rows(&source.bytes);
                let data = (source.definition as usize + 1) * 4096;
                let mut previous = source.bytes;
                let mut old = target.name();
                for name in ["ExpandedRenamedColumn", "X", "x", "x", target.name()] {
                    let after = changes::rename(
                        &previous,
                        source.definition,
                        source.catalog_root,
                        old,
                        name,
                    );
                    if name == old {
                        assert_eq!(after, previous);
                    }
                    observe(&after, &expected);
                    assert_eq!(
                        &after[data..data + 2 * 4096],
                        &previous[data..data + 2 * 4096]
                    );
                    previous = after;
                    old = name;
                }
                let dropped =
                    changes::drop_tail(&previous, source.definition, source.catalog_root, old);
                let mut survivors = expected;
                for row in &mut survivors {
                    row.pop();
                }
                observe(&dropped, &survivors);
                assert_eq!(
                    &dropped[data..data + 2 * 4096],
                    &previous[data..data + 2 * 4096]
                );
                let page = source.definition as usize * 4096;
                let high = u16::from_le_bytes(dropped[page + 41..page + 43].try_into().unwrap());
                let readded =
                    fields::after(&dropped, source.definition, source.catalog_root, target);
                let active =
                    u16::from_le_bytes(readded[page + 45..page + 47].try_into().unwrap()) as usize;
                let field = page + 75 + (active - 1) * 25;
                assert_eq!(
                    u16::from_le_bytes(readded[field + 5..field + 7].try_into().unwrap()),
                    high
                );
                for row in &mut survivors {
                    row.push(Value::Null);
                }
                observe(&readded, &survivors);
                assert_eq!(
                    &readded[data..data + 2 * 4096],
                    &previous[data..data + 2 * 4096]
                );
            }
        }
    }
}
