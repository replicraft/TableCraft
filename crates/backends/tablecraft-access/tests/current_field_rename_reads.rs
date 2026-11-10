//! Independent retained RENAME images preserve sparse historical row semantics.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{inspect_table_schema, read_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        self as dropped, readdition::Target,
    },
    saved_field_rename::fitting::current_history as raw,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1517);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

#[test]
fn retained_rename_reads_preserve_sparse_identities_and_every_payload_history() {
    for case in dropped::CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                let expected = raw::expected(case, target, filled);
                let mut before = raw::source(case, target, filled);
                let original = inspect_table_schema(
                    &mut Cursor::new(&before),
                    SCOPE,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let original_identity = original.columns().last().unwrap();
                let mut old_name = target.name();
                for new_name in ["ALongerRenamedField", "X", "x", "x"] {
                    let after = raw::rename(&before, old_name, new_name);
                    let current = inspect_table_schema(
                        &mut Cursor::new(&after),
                        SCOPE,
                        "Table1",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let selected = current.columns().last().unwrap();
                    assert_eq!(selected.name(), new_name);
                    assert_eq!(selected.number(), original_identity.number());
                    assert_eq!(current.declared_rows(), original.declared_rows());
                    assert_eq!(current.columns().len(), original.columns().len());
                    assert_eq!(&after[8 * PAGE..10 * PAGE], &before[8 * PAGE..10 * PAGE]);
                    let slots = word(&before, 10 * PAGE + 12);
                    let frontier = word(&before, 10 * PAGE + 14 + (slots - 1) * 2);
                    assert_eq!(
                        &after[10 * PAGE + frontier..11 * PAGE],
                        &before[10 * PAGE + frontier..11 * PAGE],
                        "old MR2/OFF/history bytes changed: {case:?}/{target:?}/{filled}",
                    );
                    if old_name == new_name {
                        assert_eq!(after, before);
                    } else {
                        assert_eq!(word(&after, 10 * PAGE + 12), slots + 1);
                    }
                    let table = read_table(
                        &mut Cursor::new(&after),
                        SCOPE,
                        "Table1",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap_or_else(|error| {
                        panic!("{case:?}/{target:?}/{filled}/{new_name}: {error:?}")
                    });
                    assert_eq!(table.rows(), expected);
                    before = after;
                    old_name = new_name;
                }
            }
        }
    }
}

#[test]
fn retained_rename_fresh_queries_use_current_names_and_literal_values() {
    for case in dropped::CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                let before = raw::source(case, target, filled);
                let after = raw::rename(&before, target.name(), "RenamedField");
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("renamed.accdb");
                fs::write(&path, &after).unwrap();
                for scope in [1518, 1519] {
                    let source = NativeSource::new(
                        fs::File::open(&path).unwrap(),
                        CatalogScope::new(scope),
                        budget(),
                        &|| false,
                    )
                    .unwrap();
                    let query = parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default())
                        .unwrap();
                    assert_eq!(
                        prepare(query.query(), &source)
                            .unwrap()
                            .execute(&source, &[], ExecutionOptions::default())
                            .unwrap()
                            .rows(),
                        raw::expected(case, target, filled),
                    );
                    let missing = parse_query(
                        &format!("SELECT [{}] FROM Table1", target.name()),
                        Default::default(),
                    )
                    .unwrap();
                    assert!(prepare(missing.query(), &source).is_err());
                    let renamed = parse_query(
                        "SELECT RenamedField FROM Table1 ORDER BY ID",
                        Default::default(),
                    )
                    .unwrap();
                    let expected: Vec<_> = raw::expected(case, target, filled)
                        .into_iter()
                        .map(|mut row| vec![row.pop().unwrap()])
                        .collect();
                    assert_eq!(
                        prepare(renamed.query(), &source)
                            .unwrap()
                            .execute(&source, &[], ExecutionOptions::default())
                            .unwrap()
                            .rows(),
                        expected,
                    );
                }
                assert_eq!(fs::read(path).unwrap(), after);
            }
        }
    }
}
