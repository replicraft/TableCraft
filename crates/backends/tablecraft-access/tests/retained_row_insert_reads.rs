//! Independent generated row appends target actual new-table pages in retained images.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        CASES, Case, readdition::Target as Field,
    },
    canonical_retained_add_table::{
        self as raw, Target,
        rows::{self, Input, Profile},
    },
    canonical_table_drop::current as dropped,
};

const PAGE: usize = 4096;
const PROFILES: [(Profile, Target, bool); 4] = [
    (Profile::NativeNumber, Target::GeneratedNumber, false),
    (Profile::NativeText, Target::GeneratedText, false),
    (Profile::DesktopNumber, Target::GeneratedNumber, true),
    (Profile::DesktopText, Target::GeneratedText, true),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn inputs(profile: Profile) -> [Input<'static>; 4] {
    match profile {
        Profile::NativeNumber | Profile::DesktopNumber => [
            Input::Omitted,
            Input::Null,
            Input::Number(123),
            Input::Number(-12),
        ],
        Profile::NativeText => [
            Input::Omitted,
            Input::Null,
            Input::Text("new label"),
            Input::Text("日本語🦀"),
        ],
        Profile::DesktopText => [
            Input::Omitted,
            Input::Null,
            Input::Text(""),
            Input::Text("日本語🦀"),
        ],
    }
}

fn observe(bytes: &[u8], name: &str, expected: &[Vec<Value>]) {
    let scope = CatalogScope::new(2021);
    assert_eq!(
        read_table(&mut Cursor::new(bytes), scope, name, &mut budget(), &|| {
            false
        })
        .unwrap()
        .rows(),
        expected,
    );
    check_table(&mut Cursor::new(bytes), scope, name, &mut budget(), &|| {
        false
    })
    .unwrap();
}

#[test]
fn retained_first_and_sequential_inserts_keep_exact_sibling_and_retired_images() {
    for case in CASES {
        for history in dropped::HISTORIES {
            let original = dropped::source(case, Field::ShortText, true, history);
            let retired = dropped::after(&original);
            for (profile, target, desktop) in PROFILES {
                for (before, name) in [
                    (original.bytes.as_slice(), "Fresh"),
                    (retired.as_slice(), "Table1"),
                ] {
                    let definition = (before.len() / PAGE) as u32;
                    let mut bytes = if desktop {
                        raw::desktop::append(before, name, target)
                    } else {
                        raw::append(before, name, target)
                    };
                    let mut expected = Vec::new();
                    for (position, input) in inputs(profile).into_iter().enumerate() {
                        let previous = bytes;
                        bytes = rows::append(&previous, definition, profile, input);
                        expected.push(rows::row(profile, input, (position + 1) as i32));
                        assert_eq!(
                            &bytes[..definition as usize * PAGE],
                            &previous[..definition as usize * PAGE]
                        );
                        observe(&bytes, name, &expected);
                    }
                }
            }
        }
    }
}

#[test]
fn two_new_tables_generate_independently_and_fresh_queries_publish_only_live_rows() {
    for name in ["Table1", "Replacement", "新しい🙂"] {
        let original = dropped::source(
            Case::MixedBinary,
            Field::ShortText,
            true,
            dropped::History::Deleted,
        );
        let retired = dropped::after(&original);
        let first_definition = (retired.len() / PAGE) as u32;
        let first = raw::desktop::append(&retired, name, Target::GeneratedNumber);
        let first = rows::append(
            &first,
            first_definition,
            Profile::DesktopNumber,
            Input::Omitted,
        );
        let second_definition = (first.len() / PAGE) as u32;
        let mut bytes = raw::desktop::append(&first, "Second", Target::GeneratedText);
        let mut first_rows = vec![vec![Value::Int32(1), Value::Int32(0)]];
        let mut second_rows = Vec::new();
        for (position, input) in inputs(Profile::DesktopText).into_iter().enumerate() {
            bytes = rows::append(&bytes, second_definition, Profile::DesktopText, input);
            second_rows.push(rows::row(
                Profile::DesktopText,
                input,
                (position + 1) as i32,
            ));
            bytes = rows::append(
                &bytes,
                first_definition,
                Profile::DesktopNumber,
                Input::Number(123),
            );
            first_rows.push(vec![Value::Int32((position + 2) as i32), Value::Int32(123)]);
            observe(&bytes, name, &first_rows);
            observe(&bytes, "Second", &second_rows);
        }
        assert_eq!(&bytes[7 * PAGE..retired.len()], &retired[7 * PAGE..]);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("inserted.accdb");
        fs::write(&path, &bytes).unwrap();
        for scope in [2022, 2023] {
            let source = NativeSource::new(
                fs::File::open(&path).unwrap(),
                CatalogScope::new(scope),
                budget(),
                &|| false,
            )
            .unwrap();
            for (table, expected) in [(name, &first_rows), ("Second", &second_rows)] {
                let query = parse_query(
                    &format!("SELECT * FROM [{table}] ORDER BY ID"),
                    Default::default(),
                )
                .unwrap();
                assert_eq!(
                    prepare(query.query(), &source)
                        .unwrap()
                        .execute(&source, &[], ExecutionOptions::default())
                        .unwrap()
                        .rows(),
                    expected
                );
            }
        }
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}
