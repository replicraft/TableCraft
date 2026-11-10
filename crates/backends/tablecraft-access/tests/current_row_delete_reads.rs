//! Independent current DELETE images retain opaque storage and generated history.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::read_table;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    current_delete as raw, current_insert,
    deleted_field::populated::{CASES, readdition::Target},
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1721);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 40_000_000)
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

#[test]
fn current_deletes_keep_logical_survivors_and_all_inactive_payload_bytes() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for renamed in [false, true] {
                    for inserted in [false, true] {
                        let source = raw::source(case, target, filled, renamed, inserted);
                        let last = *source.roots.last().unwrap();
                        let requests = [
                            Vec::new(),
                            vec![source.roots[0]],
                            vec![source.roots[1]],
                            vec![last],
                            vec![source.roots[0], last],
                            source.roots.to_vec(),
                        ];
                        let frontier = word(
                            &source.bytes,
                            8 * PAGE + 14 + (word(&source.bytes, 8 * PAGE + 12) - 1) * 2,
                        ) & 0x1fff;
                        for removed in requests {
                            let after = raw::after(&source.bytes, &removed);
                            let mut reversed = removed.to_vec();
                            reversed.reverse();
                            assert_eq!(raw::after(&source.bytes, &reversed), after);
                            assert_eq!(&after[10 * PAGE..], &source.bytes[10 * PAGE..]);
                            assert_eq!(&after[..7 * PAGE], &source.bytes[..7 * PAGE]);
                            assert_eq!(
                                &after[7 * PAGE + 20..7 * PAGE + 24],
                                &source.bytes[7 * PAGE + 20..7 * PAGE + 24]
                            );
                            assert_eq!(
                                &after[7 * PAGE + 67..7 * PAGE + 71],
                                &source.bytes[7 * PAGE + 67..7 * PAGE + 71]
                            );
                            assert_eq!(
                                &after[8 * PAGE + frontier..9 * PAGE],
                                &source.bytes[8 * PAGE + frontier..9 * PAGE]
                            );
                            assert_eq!(
                                &after[8 * PAGE + 2..8 * PAGE + 4],
                                &source.bytes[8 * PAGE + 2..8 * PAGE + 4]
                            );
                            assert_eq!(
                                word(&after, 8 * PAGE + 12),
                                word(&source.bytes, 8 * PAGE + 12)
                            );
                            let survivors = source.rows.len() - removed.len();
                            assert_eq!(
                                &after[9 * PAGE + 480 + survivors * 9..10 * PAGE],
                                &source.bytes[9 * PAGE + 480 + survivors * 9..10 * PAGE]
                            );
                            let table = read_table(&mut Cursor::new(&after),SCOPE,"Table1",&mut budget(),&|| false)
                                .unwrap_or_else(|error| panic!("{case:?}/{target:?}/{filled}/{renamed}/{inserted}/{removed:?}: {error:?}"));
                            assert!(
                                table
                                    .rows()
                                    .iter()
                                    .map(Vec::as_slice)
                                    .eq(source.survivors(&removed))
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn delete_all_then_insert_keeps_next_generated_id_and_queries_only_new_membership() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for inserted in [false, true] {
                    let source = raw::source(case, target, filled, true, inserted);
                    let empty = raw::after(&source.bytes, &source.roots);
                    assert!(
                        read_table(
                            &mut Cursor::new(&empty),
                            SCOPE,
                            "Table1",
                            &mut budget(),
                            &|| false
                        )
                        .unwrap()
                        .rows()
                        .is_empty()
                    );
                    assert_eq!(raw::after(&empty, &[]), empty);
                    let after = current_insert::append(
                        &empty,
                        case,
                        target,
                        current_insert::Input::Omitted,
                    );
                    let expected = vec![current_insert::row(
                        case,
                        target,
                        current_insert::Input::Omitted,
                        source.next_id,
                    )];
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
                    let directory = tempfile::tempdir().unwrap();
                    let path = directory.path().join("reinserted.accdb");
                    fs::write(&path, &after).unwrap();
                    for scope in [1722, 1723] {
                        let source = NativeSource::new(
                            fs::File::open(&path).unwrap(),
                            CatalogScope::new(scope),
                            budget(),
                            &|| false,
                        )
                        .unwrap();
                        let query =
                            parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default())
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
                    assert_eq!(fs::read(path).unwrap(), after);
                }
            }
        }
    }
}
