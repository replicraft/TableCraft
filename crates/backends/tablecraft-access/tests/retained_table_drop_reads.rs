//! Independent retained table deletion keeps siblings and permits dead-name reuse.
use std::{collections::BTreeSet, fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, inspect_catalog, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{
        self as added, Target,
        rows::{self, Input, Profile},
    },
    canonical_table_drop::retained::{self as raw, Case},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn names(bytes: &[u8]) -> BTreeSet<String> {
    inspect_catalog(
        &mut Cursor::new(bytes),
        CatalogScope::new(3611),
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
fn retained_table_drop_keeps_storage_and_sibling_catalog_row_identity() {
    for case in raw::CASES {
        let source = raw::source(case);
        let after = raw::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            &source.roots,
        );
        let mut expected = names(&source.bytes);
        assert!(expected.remove("Table1"));
        assert_eq!(names(&after), expected);
        assert_eq!(after.len(), source.bytes.len());
        assert_eq!(&after[..2 * 4096], &source.bytes[..2 * 4096]);
        assert_eq!(
            &after[3 * 4096..4 * 4096],
            &source.bytes[3 * 4096..4 * 4096]
        );
        assert_eq!(
            &after[5 * 4096..7 * 4096],
            &source.bytes[5 * 4096..7 * 4096]
        );
        let other = read_table(
            &mut Cursor::new(&after),
            CatalogScope::new(3612),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected_other = if matches!(case, Case::SecondText) {
            vec![vec![Value::Int32(1), Value::Int32(99)]]
        } else {
            Vec::new()
        };
        assert_eq!(other.rows(), expected_other);
        check_table(
            &mut Cursor::new(&after),
            CatalogScope::new(3613),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            read_table(
                &mut Cursor::new(&after),
                CatalogScope::new(3614),
                "Table1",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        let td = source.definition as usize * 4096;
        assert_eq!(
            &after[td + 20..td + 4096],
            &source.bytes[td + 20..td + 4096]
        );
    }
}

#[test]
fn retained_dead_name_reuse_assigns_fresh_pages_and_new_generator_after_reopen() {
    for case in raw::CASES {
        let source = raw::source(case);
        let deleted = raw::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            &source.roots,
        );
        let definition = (deleted.len() / 4096) as u32;
        let recreated = added::desktop::append(&deleted, "Table1", Target::GeneratedNumber);
        let inserted = rows::append(
            &recreated,
            definition,
            Profile::DesktopNumber,
            Input::Omitted,
        );
        assert_eq!(&inserted[7 * 4096..deleted.len()], &deleted[7 * 4096..]);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recreated.accdb");
        fs::write(&path, &deleted).unwrap();
        let old = NativeSource::new(
            fs::File::open(&path).unwrap(),
            CatalogScope::new(3615),
            budget(),
            &|| false,
        )
        .unwrap();
        let query = parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default()).unwrap();
        assert!(prepare(query.query(), &old).is_err());
        drop(old);
        fs::write(&path, &inserted).unwrap();
        let fresh = NativeSource::new(
            fs::File::open(&path).unwrap(),
            CatalogScope::new(3616),
            budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            prepare(query.query(), &fresh)
                .unwrap()
                .execute(&fresh, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int32(1), Value::Int32(0)]]
        );
        check_table(
            &mut Cursor::new(&inserted),
            CatalogScope::new(3617),
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(fs::read(path).unwrap(), inserted);
    }
}
