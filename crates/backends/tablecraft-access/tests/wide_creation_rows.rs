//! Independent wide declarations retain exact defaults, mask tails and indexes.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, read_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::fields::{self, Case},
    canonical_wide_table_creation::{
        self as raw, appended,
        rows::{self, LastFieldInput},
    },
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn all_255_fields_and_last_field_presence_survive_fresh_reads_queries_and_index_checks() {
    let initial = raw::source();
    let second = appended::source(&initial.after, "AppendedWide");
    let retained = fields::source(Case::FirstNumber);
    let beside_retained = appended::source(&retained.bytes, "AppendedWide");
    for (source, table) in [
        (initial, "Table1"),
        (second, "AppendedWide"),
        (beside_retained, "AppendedWide"),
    ] {
        let scope = CatalogScope::new(4521);
        let siblings = if table == "AppendedWide" {
            Some(
                read_table(
                    &mut Cursor::new(&source.after),
                    scope,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            )
        } else {
            None
        };
        for input in [
            LastFieldInput::Omitted,
            LastFieldInput::Value(123),
            LastFieldInput::Null,
        ] {
            let after = rows::after(&source.after, &source, input);
            let expected = vec![rows::values(input)];
            let read = read_table(
                &mut Cursor::new(&after),
                scope,
                table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), expected);
            check_table(
                &mut Cursor::new(&after),
                scope,
                table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            if let Some(sibling) = &siblings {
                let observed = read_table(
                    &mut Cursor::new(&after),
                    scope,
                    "Table1",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(observed.rows(), sibling.rows());
            }
            for page in 0..source.after.len() / 4096 {
                if page != source.definition_pages[0] as usize
                    && page != source.data_page as usize
                    && page != source.index_page as usize
                {
                    assert_eq!(
                        &after[page * 4096..(page + 1) * 4096],
                        &source.after[page * 4096..(page + 1) * 4096],
                        "protected page {page}"
                    );
                }
            }
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("wide.accdb");
            fs::write(&path, &after).unwrap();
            let fresh = NativeSource::new(
                fs::File::open(&path).unwrap(),
                CatalogScope::new(4522),
                budget(),
                &|| false,
            )
            .unwrap();
            let sql = parse_query(
                &format!("SELECT * FROM [{table}] ORDER BY ID"),
                Default::default(),
            )
            .unwrap();
            assert_eq!(
                prepare(sql.query(), &fresh)
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
