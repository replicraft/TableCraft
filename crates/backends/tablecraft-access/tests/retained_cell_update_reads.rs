//! Independent target-local cell replacements retain indexes and sibling storage.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_insert,
        deleted_field::populated::{
            CASES, Case,
            readdition::{self, Target as Field},
        },
    },
    canonical_retained_add_table::{
        self as raw, Target,
        rows::{self, Input, Profile},
        updates,
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

fn observe(bytes: &[u8], name: &str, expected: &[Vec<Value>]) {
    let scope = CatalogScope::new(3221);
    assert_eq!(
        read_table(&mut Cursor::new(bytes), scope, name, &mut budget(), &|| {
            false
        })
        .unwrap()
        .rows(),
        expected
    );
    check_table(&mut Cursor::new(bytes), scope, name, &mut budget(), &|| {
        false
    })
    .unwrap();
}

fn complement(before: &[u8], after: &[u8], definition: u32) {
    let start = (definition as usize + 1) * PAGE;
    assert_eq!(before.len(), after.len());
    assert_eq!(&before[..start], &after[..start]);
    assert_eq!(&before[start + PAGE..], &after[start + PAGE..]);
}

#[test]
fn retained_companion_changes_keep_native_ids_counters_indexes_and_all_siblings() {
    for case in CASES {
        let original = dropped::source(case, Field::ShortText, true, dropped::History::Inserted);
        let retired = dropped::after(&original);
        for before in [original.bytes.as_slice(), retired.as_slice()] {
            for (profile, target, desktop) in PROFILES {
                let definition = (before.len() / PAGE) as u32;
                let declared = if desktop {
                    raw::desktop::append(before, "Edited", target)
                } else {
                    raw::append(before, "Edited", target)
                };
                let numeric = matches!(profile, Profile::NativeNumber | Profile::DesktopNumber);
                let seeds = if numeric {
                    [Input::Omitted, Input::Number(42), Input::Number(-7)]
                } else {
                    [
                        Input::Omitted,
                        Input::Text("first"),
                        Input::Text("日本語🦀"),
                    ]
                };
                let mut bytes = declared;
                let mut expected = Vec::new();
                for (root, input) in seeds.into_iter().enumerate() {
                    bytes = rows::append(&bytes, definition, profile, input);
                    expected.push(rows::row(profile, input, (root + 1) as i32));
                }
                bytes = raw::append(&bytes, "Sibling", Target::GeneratedNumber);
                if numeric {
                    for (root, value) in [
                        (0, Some(123)),
                        (1, None),
                        (2, Some(-7)),
                        (0, None),
                        (0, None),
                        (0, Some(0)),
                    ] {
                        let prior = bytes;
                        bytes = updates::number(&prior, definition, root, value);
                        let value = value.map_or(Value::Null, Value::Int32);
                        if expected[root][1] == value {
                            assert_eq!(bytes, prior);
                        }
                        expected[root][1] = value;
                        complement(&prior, &bytes, definition);
                        observe(&bytes, "Edited", &expected);
                    }
                } else {
                    let empty = if desktop { "" } else { "x" };
                    for (root, value) in [
                        (0, Some("a longer replacement")),
                        (1, None),
                        (2, Some("日本語🦀")),
                        (0, Some("x")),
                        (0, Some(empty)),
                        (0, None),
                        (0, None),
                        (0, Some("a much longer second growth 日本語🦀")),
                    ] {
                        let prior = bytes;
                        bytes = updates::text(&prior, definition, root, value);
                        let value = value.map_or(Value::Null, |text| Value::Text(text.into()));
                        if expected[root][1] == value {
                            assert_eq!(bytes, prior);
                        }
                        expected[root][1] = value;
                        complement(&prior, &bytes, definition);
                        observe(&bytes, "Edited", &expected);
                    }
                    bytes = updates::with_opaque_text_null(&bytes, definition, 2);
                    expected[2][1] = Value::Null;
                    let untouched = updates::text(&bytes, definition, 2, None);
                    assert_eq!(untouched, bytes);
                    observe(&bytes, "Edited", &expected);
                }
                observe(&bytes, "Sibling", &[]);
            }
        }
    }
}

#[test]
fn separate_new_tables_update_independently_and_fresh_queries_keep_null_and_unicode() {
    let original = dropped::source(
        Case::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Deleted,
    );
    let retired = dropped::after(&original);
    let first = raw::desktop::append(&retired, "Numbers", Target::GeneratedNumber);
    let first = rows::append(&first, 11, Profile::DesktopNumber, Input::Number(17));
    let second = raw::desktop::append(&first, "Words", Target::GeneratedText);
    let second = rows::append(&second, 14, Profile::DesktopText, Input::Text("first"));
    let number_changed = updates::number(&second, 11, 0, None);
    let bytes = updates::text(&number_changed, 14, 0, Some("日本語🦀 larger text"));
    assert_eq!(
        bytes,
        updates::number(
            &updates::text(&second, 14, 0, Some("日本語🦀 larger text")),
            11,
            0,
            None
        )
    );
    assert_eq!(&bytes[7 * PAGE..retired.len()], &retired[7 * PAGE..]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("changed.accdb");
    fs::write(&path, &bytes).unwrap();
    for scope in [3222, 3223] {
        let source = NativeSource::new(
            fs::File::open(&path).unwrap(),
            CatalogScope::new(scope),
            budget(),
            &|| false,
        )
        .unwrap();
        for (table, expected) in [
            ("Numbers", vec![vec![Value::Int32(1), Value::Null]]),
            (
                "Words",
                vec![vec![
                    Value::Int32(1),
                    Value::Text("日本語🦀 larger text".into()),
                ]],
            ),
        ] {
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

#[test]
fn original_historical_text_change_commutes_with_appending_a_live_sibling() {
    for case in CASES {
        let original = current_insert::source(case, Field::ShortText, true, true);
        let sibling = raw::desktop::append(&original, "More", Target::GeneratedNumber);
        for value in [
            Some("longer replacement 日本語🦀"),
            Some("x"),
            Some(""),
            None,
        ] {
            let changed = readdition::replace_text(&original, 0, value);
            let expected = raw::desktop::append(&changed, "More", Target::GeneratedNumber);
            assert_eq!(&expected[11 * PAGE..], &sibling[11 * PAGE..]);
            assert_eq!(&expected[..7 * PAGE], &sibling[..7 * PAGE]);
            let table = read_table(
                &mut Cursor::new(&expected),
                CatalogScope::new(3224),
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                table.rows()[0].last().unwrap(),
                &value.map_or(Value::Null, |text| Value::Text(text.into()))
            );
            observe(&expected, "More", &[]);
        }
    }
}

#[test]
fn appended_table_updates_follow_actual_definition_and_row_page_above_sixty_three() {
    let original = dropped::source(
        Case::TextPresent,
        Field::ShortText,
        true,
        dropped::History::Fields,
    );
    let mut bytes = dropped::after(&original);
    let mut definition = 0;
    for ordinal in 0..19 {
        definition = (bytes.len() / PAGE) as u32;
        bytes = raw::append(&bytes, &format!("T{ordinal}"), Target::GeneratedNumber);
    }
    assert!(definition > 63);
    bytes = rows::append(&bytes, definition, Profile::NativeNumber, Input::Number(17));
    let after = updates::number(&bytes, definition, 0, Some(i32::MIN));
    complement(&bytes, &after, definition);
    observe(
        &after,
        "T18",
        &[vec![Value::Int32(1), Value::Int32(i32::MIN)]],
    );
}
