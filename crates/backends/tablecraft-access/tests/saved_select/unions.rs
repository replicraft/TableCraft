use super::*;
use tablecraft_access::{NativeSource, reconstruct_saved_read};
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, prepare};

fn part(text: &str, first: bool) -> Fragment<'_> {
    Fragment {
        expression: Some(text),
        name2: Some(if first { "X7YZ_____1" } else { "X7YZ_____2" }),
        ..row(5)
    }
}
fn kind() -> Fragment<'static> {
    Fragment {
        flag: Some(9),
        ..row(1)
    }
}
fn compile_read(rows: &[Fragment<'_>], flags: u32) -> Result<tablecraft_access::SavedRead, E> {
    reconstruct_saved_read(
        definition(rows, flags),
        ParseLimits::default(),
        &mut budget(),
        &|| false,
    )
}
#[test]
fn native_labels_duplicate_modes_parameters_and_final_order_preserve_owned_syntax() {
    let mut parameter = row(2);
    parameter.name1 = Some("Floor");
    parameter.flag = Some(4);
    let mut flags = row(3);
    flags.flag = Some(2 | 4);
    let mut sort = row(11);
    sort.expression = Some("[N]");
    sort.name1 = Some("D");
    let rows = [
        kind(),
        parameter,
        flags,
        part("SELECT [Floor] AS N UNION ALL SELECT 1", false),
        part("SELECT 0 AS N -- trailing comment", true),
        sort,
    ];
    let saved = compile_read(&rows, 0x80000080).unwrap();
    let expected=parse_query("PARAMETERS Floor LONG; SELECT 0 AS N UNION SELECT [Floor] AS N UNION ALL SELECT 1 ORDER BY N DESC WITH OWNERACCESS OPTION",ParseLimits::default()).unwrap();
    assert_eq!(saved.query(), expected.query());
    assert_eq!(saved.definition().origin().flags(), 0x80000080);
    assert_eq!(saved.definition().rows()[3].name2(), Some("X7YZ_____2"));
    assert!(matches!(
        reconstruct_saved_select(
            definition(&rows, 0x80),
            Default::default(),
            &mut budget(),
            &|| false
        ),
        Err(E::Unsupported)
    ));
    let all = [kind(), part("TABLE Items", true), part("Saved", false)];
    let saved = compile_read(&all, 0x80).unwrap();
    assert_eq!(
        saved.query(),
        parse_query("TABLE Items UNION ALL Saved", Default::default())
            .unwrap()
            .query()
    );
}
#[test]
fn saved_unions_execute_and_resolve_as_sources_under_existing_query_contracts() {
    for code in [1, 2, 5, 6] {
        for (flag, second, expected) in [
            (
                0,
                "SELECT 1 AS N",
                vec![vec![Value::Int64(1)], vec![Value::Int64(1)]],
            ),
            (2, "SELECT 1 AS N", vec![vec![Value::Int64(1)]]),
            (
                0,
                "SELECT 2 AS N UNION SELECT 2",
                vec![vec![Value::Int64(1)], vec![Value::Int64(2)]],
            ),
        ] {
            let mut flags = row(3);
            flags.flag = Some(flag);
            let mut bytes = database_with_query_flags(
                &[
                    kind(),
                    flags,
                    part("SELECT 1 AS N", true),
                    part(second, false),
                ],
                0x80,
            );
            bytes[20] = code;
            if code == 1 {
                bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
            }
            let original = bytes.clone();
            let mut source = NativeSource::new(
                Cursor::new(bytes.as_slice()),
                CatalogScope::new(7),
                budget(),
                &|| false,
            )
            .unwrap();
            let saved = source
                .read_saved_query("Saved", Default::default())
                .unwrap();
            let result = prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap();
            assert_eq!(result.rows(), expected);
            let query = parse_query("SELECT * FROM Saved", Default::default()).unwrap();
            let result = prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap();
            assert_eq!(result.rows(), expected);
            drop(source);
            assert_eq!(bytes, original);
        }
    }
}
#[test]
fn missing_duplicate_unknown_or_conflicting_union_metadata_is_rejected() {
    for rows in [
        vec![part("SELECT 1", true), part("SELECT 2", false)],
        vec![kind(), part("SELECT 1", true)],
        vec![
            kind(),
            part("SELECT 1", true),
            part("SELECT 2", true),
            part("SELECT 3", false),
        ],
        vec![
            Fragment {
                flag: Some(1),
                ..row(1)
            },
            part("SELECT 1", true),
            part("SELECT 2", false),
        ],
        vec![
            kind(),
            Fragment {
                flag: Some(1),
                ..row(3)
            },
            part("SELECT 1", true),
            part("SELECT 2", false),
        ],
        vec![
            kind(),
            Fragment {
                name1: Some("ignored"),
                ..part("SELECT 1", true)
            },
            part("SELECT 2", false),
        ],
        vec![
            kind(),
            part("SELECT 1", true),
            Fragment {
                name2: Some("x7yz_____2"),
                ..part("SELECT 2", false)
            },
        ],
        vec![
            kind(),
            part("SELECT 1", true),
            part("SELECT 2", false),
            column("3"),
        ],
        vec![
            kind(),
            part("SELECT 1", true),
            part("SELECT 2", false),
            Fragment {
                extra: Some(0),
                ..row(255)
            },
        ],
    ] {
        assert!(compile_read(&rows, 0x80).is_err());
    }
    for family in [0x20, 0x30, 0x40, 0x50, 0x60, 0x70] {
        assert!(matches!(
            compile_read(
                &[kind(), part("SELECT 1", true), part("SELECT 2", false)],
                family
            ),
            Err(E::Unsupported)
        ));
    }
}
#[test]
fn fragment_boundaries_do_not_repair_or_hide_action_clauses_or_statement_injection() {
    for text in [
        "SELECT 1 ORDER BY 1",
        "SELECT 1 WITH OWNERACCESS OPTION",
        "PARAMETERS P LONG; SELECT P",
        "DELETE FROM Items",
        "SELECT 1 INTO NewTable",
        "SELECT 1; SELECT 2",
        "SELECT 1 /*",
        "SELECT (1",
        "SELECT 1; -- end",
    ] {
        for first in [true, false] {
            let rows = [
                kind(),
                part(if first { text } else { "SELECT 0" }, true),
                part(if first { "SELECT 0" } else { text }, false),
            ];
            assert!(compile_read(&rows, 0x80).is_err(), "{first} {text}");
        }
    }
}
#[test]
fn union_work_bytes_rows_parser_limits_and_cancellation_prevent_publication() {
    let rows = [kind(), part("SELECT 1", true), part("SELECT 2", false)];
    assert!(matches!(
        reconstruct_saved_read(
            definition(&rows, 0x80),
            Default::default(),
            &mut budget(),
            &|| true
        ),
        Err(E::Cancelled)
    ));
    let calls = Cell::new(0usize);
    reconstruct_saved_read(
        definition(&rows, 0x80),
        Default::default(),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in [1, calls.get() / 2, calls.get()] {
        let seen = Cell::new(0usize);
        assert!(matches!(
            reconstruct_saved_read(
                definition(&rows, 0x80),
                Default::default(),
                &mut budget(),
                &|| {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                }
            ),
            Err(E::Cancelled)
        ));
    }
    for limits in [
        ParseLimits {
            max_bytes: 20,
            ..Default::default()
        },
        ParseLimits {
            max_tokens: 5,
            ..Default::default()
        },
    ] {
        assert!(
            reconstruct_saved_read(definition(&rows, 0x80), limits, &mut budget(), &|| false)
                .is_err()
        );
    }
    for mut budget in [
        ResourceBudget::new(0, 100, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 100, 0),
    ] {
        assert!(matches!(
            reconstruct_saved_read(
                definition(&rows, 0x80),
                Default::default(),
                &mut budget,
                &|| false
            ),
            Err(E::Limit(_))
        ));
    }
}
