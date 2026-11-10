//! Independent append images keep retired storage and current live siblings intact.
use std::{collections::BTreeSet, fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{inspect_catalog, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        CASES, Case, readdition::Target as Field,
    },
    canonical_retained_add_table::{self as raw, Target},
    canonical_table_drop::current as dropped,
};

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn names(bytes: &[u8]) -> BTreeSet<String> {
    inspect_catalog(
        &mut Cursor::new(bytes),
        CatalogScope::new(1921),
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

fn preserve_old_metadata(before: &[u8], after: &[u8]) {
    for number in [3, 4] {
        let page = &before[number * PAGE..(number + 1) * PAGE];
        let updated = &after[number * PAGE..(number + 1) * PAGE];
        let count = word(page, 12);
        let old_frontier = 14 + 2 * count + word(page, 2);
        assert_eq!(&updated[14..14 + 2 * count], &page[14..14 + 2 * count]);
        assert_eq!(&updated[old_frontier..], &page[old_frontier..]);
    }
    assert_eq!(&after[7 * PAGE..before.len()], &before[7 * PAGE..]);
    assert_eq!(&after[..2 * PAGE], &before[..2 * PAGE]);
    assert_eq!(&after[5 * PAGE..7 * PAGE], &before[5 * PAGE..7 * PAGE]);
}

fn verify(before: &[u8], name: &str, target: Target) -> Vec<u8> {
    let after = raw::append(before, name, target);
    preserve_old_metadata(before, &after);
    let mut expected = names(before);
    assert!(expected.insert(name.into()));
    assert_eq!(names(&after), expected);
    let table = read_table(
        &mut Cursor::new(&after),
        CatalogScope::new(1922),
        name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.schema().definition_page() as usize,
        before.len() / PAGE
    );
    assert_eq!(table.schema().declared_rows(), 0);
    assert!(table.rows().is_empty());
    let columns = table
        .columns()
        .iter()
        .map(|column| column.name())
        .collect::<Vec<_>>();
    assert_eq!(
        columns,
        if matches!(target, Target::GeneratedText) {
            ["Id", "Name"]
        } else {
            ["Id", "Score"]
        }
    );
    after
}

#[test]
fn retained_appends_accept_live_and_deleted_histories_without_reusing_old_storage() {
    for case in CASES {
        for history in dropped::HISTORIES {
            let source = dropped::source(case, Field::ShortText, true, history);
            let deleted = dropped::after(&source);
            for target in raw::TARGETS {
                for (before, name) in [
                    (source.bytes.as_slice(), "Fresh"),
                    (deleted.as_slice(), "Table1"),
                ] {
                    verify(before, name, target);
                }
            }
        }
    }
}

#[test]
fn repeated_appends_cross_inline_map_base_windows_and_keep_off_and_dead_roots() {
    let source = dropped::source(
        Case::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Deleted,
    );
    let retired = dropped::after(&source);
    let mut bytes = retired.to_vec();
    let mut expected_names = Vec::new();
    for ordinal in 0..19 {
        let name = format!("R{ordinal}");
        let definition = bytes.len() / PAGE;
        let map_slot = word(&bytes, 3 * PAGE + 12);
        let after = verify(&bytes, &name, Target::GeneratedNumber);
        for (extra, member) in [definition + 1, definition + 1, definition + 2]
            .into_iter()
            .enumerate()
        {
            let offset = word(&after, 3 * PAGE + 14 + 2 * (map_slot + extra));
            let record = &after[3 * PAGE + offset..3 * PAGE + offset + 13];
            let base = u32::from_le_bytes(record[1..5].try_into().unwrap()) as usize;
            assert_eq!(base, member / 64 * 64);
            assert_eq!(record[5 + (member - base) / 8], 1 << ((member - base) % 8));
        }
        bytes = after;
        expected_names.push(name);
    }
    assert!(bytes.len() > 64 * PAGE);
    assert_eq!(&bytes[7 * PAGE..retired.len()], &retired[7 * PAGE..]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("replacement.accdb");
    fs::write(&path, &bytes).unwrap();
    for scope in [1923, 1924] {
        let source = NativeSource::new(
            fs::File::open(&path).unwrap(),
            CatalogScope::new(scope),
            budget(),
            &|| false,
        )
        .unwrap();
        for name in &expected_names {
            let query = parse_query(
                &format!("SELECT Count(*) FROM [{name}]"),
                Default::default(),
            )
            .unwrap();
            assert_eq!(
                prepare(query.query(), &source)
                    .unwrap()
                    .execute(&source, &[], ExecutionOptions::default())
                    .unwrap()
                    .rows(),
                &[vec![Value::Int32(0)]],
            );
        }
        let old = parse_query("SELECT * FROM Table1", Default::default()).unwrap();
        assert!(prepare(old.query(), &source).is_err());
    }
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn replacement_name_variants_reopen_with_distinct_current_native_identity() {
    let original = dropped::source(
        Case::TextPresent,
        Field::ShortText,
        true,
        dropped::History::Fields,
    );
    let before = dropped::after(&original);
    for name in ["Table1", "table1", "Replacement", "新しい🙂"] {
        for target in [Target::GeneratedNumber, Target::GeneratedText] {
            let after = verify(&before, name, target);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("new.accdb");
            fs::write(&path, &after).unwrap();
            let source = NativeSource::new(
                fs::File::open(&path).unwrap(),
                CatalogScope::new(1925),
                budget(),
                &|| false,
            )
            .unwrap();
            let query =
                parse_query(&format!("SELECT * FROM [{name}]"), Default::default()).unwrap();
            assert!(
                prepare(query.query(), &source)
                    .unwrap()
                    .execute(&source, &[], ExecutionOptions::default())
                    .unwrap()
                    .rows()
                    .is_empty()
            );
            assert_eq!(fs::read(path).unwrap(), after);
        }
    }
}
