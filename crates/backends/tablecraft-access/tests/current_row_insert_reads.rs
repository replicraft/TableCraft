//! Independent current INSERT images preserve historical roots and native identities.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::read_table;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_insert as raw,
        deleted_field::populated::{self as dropped, readdition::Target},
    },
    saved_field_rename::fitting::current_history,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1641);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 40_000_000)
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

#[test]
fn sequential_current_inserts_use_literal_defaults_and_independent_physical_roots() {
    for case in dropped::CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for renamed in [false, true] {
                    let mut bytes = raw::source(case, target, filled, renamed);
                    let metadata = bytes[..8 * PAGE].to_vec();
                    let history = bytes[10 * PAGE..].to_vec();
                    let mut expected = current_history::expected(case, target, filled);
                    let inputs = match target {
                        Target::Number => raw::NUMBER_INPUTS.as_slice(),
                        Target::ShortText => raw::TEXT_INPUTS.as_slice(),
                    };
                    for (position, input) in inputs.iter().copied().enumerate() {
                        let first = if matches!(case, dropped::Case::NumberPresent) {
                            5
                        } else {
                            4
                        };
                        let id = first + position as i32;
                        let physical = word(&bytes, 8 * PAGE + 12);
                        let frontier = word(&bytes, 8 * PAGE + 14 + (physical - 1) * 2) & 0x1fff;
                        let before = bytes;
                        bytes = raw::append(&before, case, target, input);
                        expected.push(raw::row(case, target, input, id));
                        assert_eq!(&bytes[10 * PAGE..], history);
                        assert_eq!(&bytes[..7 * PAGE], &metadata[..7 * PAGE]);
                        assert_eq!(
                            &bytes[8 * PAGE + frontier..9 * PAGE],
                            &before[8 * PAGE + frontier..9 * PAGE]
                        );
                        assert_eq!(word(&bytes, 8 * PAGE + 12), physical + 1);
                        let active = expected.len() - 1;
                        let entry = 9 * PAGE + 480 + active * 9;
                        assert_eq!(
                            u32::from_be_bytes(bytes[entry + 5..entry + 9].try_into().unwrap()),
                            (8 << 8) | physical as u32,
                        );
                        let table = read_table(
                            &mut Cursor::new(&bytes),
                            SCOPE,
                            "Table1",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{target:?}/{filled}/{renamed}/{input:?}: {error:?}")
                        });
                        assert_eq!(
                            table.rows(),
                            expected,
                            "{case:?}/{target:?}/{filled}/{renamed}/{input:?}"
                        );
                    }
                    let directory = tempfile::tempdir().unwrap();
                    let path = directory.path().join("inserted.accdb");
                    fs::write(&path, &bytes).unwrap();
                    let source =
                        NativeSource::new(fs::File::open(&path).unwrap(), SCOPE, budget(), &|| {
                            false
                        })
                        .unwrap();
                    let query = parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default())
                        .unwrap();
                    assert_eq!(
                        prepare(query.query(), &source)
                            .unwrap()
                            .execute(&source, &[], ExecutionOptions::default())
                            .unwrap()
                            .rows(),
                        expected
                    );
                    let count =
                        parse_query("SELECT Count(*) FROM Table1", Default::default()).unwrap();
                    assert_eq!(
                        prepare(count.query(), &source)
                            .unwrap()
                            .execute(&source, &[], ExecutionOptions::default())
                            .unwrap()
                            .rows(),
                        &[vec![Value::Int64(expected.len() as i64)]]
                    );
                    drop(source);
                    assert_eq!(fs::read(path).unwrap(), bytes);
                }
            }
        }
    }
}

#[test]
fn explicit_binary_sibling_is_distinct_from_omitted_null_after_current_rename() {
    let case = dropped::Case::MixedBinary;
    let target = Target::ShortText;
    let before = raw::source(case, target, true, true);
    let after = raw::append_with_binary(&before, case, target, raw::Input::Unicode, &[0, 255, 33]);
    let mut expected = current_history::expected(case, target, true);
    expected.push(raw::row_with_binary(
        case,
        target,
        raw::Input::Unicode,
        4,
        &[0, 255, 33],
    ));
    assert_eq!(
        read_table(
            &mut Cursor::new(&after),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        expected
    );
    assert_eq!(&before[10 * PAGE..], &after[10 * PAGE..]);
}
