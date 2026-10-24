use super::joins::{join, source};
use super::*;
use tablecraft_access::NativeSource;
use tablecraft_ace::QueryAttribute;
use tablecraft_core::values::Value;
use tablecraft_query::prepare;

fn derived<'a>(text: &'a str, alias: Option<&'a str>) -> Fragment<'a> {
    Fragment {
        expression: Some(text),
        name2: alias,
        ..row(5)
    }
}

#[test]
fn derived_sources_preserve_owned_reads_aliases_nesting_union_and_anonymous_roots() {
    for (text, alias, sql) in [
        (
            "SELECT ID FROM Items",
            Some("d"),
            "SELECT 1 FROM (SELECT ID FROM Items) AS d",
        ),
        (
            "SELECT 1 AS N UNION ALL SELECT 2",
            Some("d"),
            "SELECT 1 FROM (SELECT 1 AS N UNION ALL SELECT 2) AS d",
        ),
        ("SELECT 1 AS N", None, "SELECT 1 FROM (SELECT 1 AS N)"),
        (
            "SELECT q.ID FROM (SELECT ID FROM Items) AS q",
            Some("d"),
            "SELECT 1 FROM (SELECT q.ID FROM (SELECT ID FROM Items) AS q) AS d",
        ),
        (
            "SELECT 1 AS N -- trailing comment",
            Some("é"),
            "SELECT 1 FROM (SELECT 1 AS N) AS é",
        ),
        (
            "SELECT NeverRunMe() AS N",
            Some("d"),
            "SELECT 1 FROM (SELECT NeverRunMe() AS N) AS d",
        ),
        (
            "SELECT ID FROM Items; ",
            Some("d"),
            "SELECT 1 FROM (SELECT ID FROM Items) AS d",
        ),
        (
            "SELECT ';' AS N; -- ; ignored",
            Some("d"),
            "SELECT 1 FROM (SELECT ';' AS N) AS d",
        ),
        (
            "SELECT 'é🦀;' AS N\t; /* ; ignored */",
            Some("d"),
            "SELECT 1 FROM (SELECT 'é🦀;' AS N) AS d",
        ),
        (
            "SELECT 1 AS N -- ; comment\n; -- final comment",
            Some("d"),
            "SELECT 1 FROM (SELECT 1 AS N) AS d",
        ),
    ] {
        let saved = compile(&[column("1"), derived(text, alias)]).unwrap();
        assert_eq!(
            saved.query(),
            parse_query(sql, Default::default()).unwrap().query()
        );
        assert_eq!(saved.definition().rows()[1].expression(), Some(text));
    }
    let saved = compile(&[
        column("1"),
        derived("SELECT 2 AS A", None),
        derived("SELECT 3 AS B", None),
    ])
    .unwrap();
    assert_eq!(
        saved.query(),
        parse_query(
            "SELECT 1 FROM (SELECT 2 AS A),(SELECT 3 AS B)",
            Default::default()
        )
        .unwrap()
        .query()
    );
}

#[test]
fn derived_sources_reject_remote_qualifiers_actions_parameters_injection_and_duplicate_labels() {
    for text in [
        "UPDATE Items SET Id=1",
        "DELETE FROM Items",
        "SELECT ID INTO Copy FROM Items",
        "TRANSFORM Count(*) SELECT Id FROM Items GROUP BY Id PIVOT Id",
        "PARAMETERS P LONG; SELECT P",
        "SELECT 1; SELECT 2",
        "SELECT 1;;",
        "SELECT 1; -- delimiter\nSELECT 2;",
        "SELECT 1) AS Injected WHERE False --",
        "SELECT (1",
        "SELECT 1) UNION SELECT (2",
        "",
    ] {
        assert!(
            compile(&[column("1"), derived(text, Some("d"))]).is_err(),
            "{text}"
        );
    }
    assert_eq!(
        compile(&[
            column("1"),
            Fragment {
                name1: Some("Items"),
                ..derived("external.accdb", Some("d"))
            }
        ])
        .err(),
        Some(E::Unsupported)
    );
    assert_eq!(
        compile(&[
            column("1"),
            source("Items", Some("d")),
            derived("SELECT 1 AS N", Some("D"))
        ])
        .err(),
        Some(E::InvalidMetadata)
    );
    assert!(
        compile(&[
            column("1"),
            derived("SELECT 1 AS N", None),
            source("Items", Some("i")),
            join(1, "missing", "i", "True")
        ])
        .is_err()
    );
}

#[test]
fn registered_terminated_source_keeps_original_metadata_and_its_remaining_parameter_refusal() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/examples/Access_Example_VBA.accdb");
    let original = std::fs::read(&path).unwrap();
    let definition = read_saved_query_definition(
        &mut Cursor::new(&original),
        CatalogScope::new(1),
        "~sq_d510~sq_dChild52",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let text = definition
        .rows()
        .iter()
        .find(|row| row.attribute() == Some(QueryAttribute::Table))
        .unwrap()
        .expression()
        .unwrap();
    assert!(text.ends_with("; "));

    // Exercise this source independently of the original implicit parameter.
    let saved = compile(&[column("1"), derived(text, Some("510"))]).unwrap();
    let expected = parse_query(
        "SELECT 1 FROM (SELECT TRAP_LCTN_TB.LOCATION_NAME, TRAP_LCTN_TB.TRAP_TYPE_ID \
         FROM TRAP_LCTN_TB WHERE (((TRAP_LCTN_TB.NACT_INDC)=False)) \
         ORDER BY TRAP_LCTN_TB.LOCATION_NAME) AS [510]",
        Default::default(),
    )
    .unwrap();
    assert_eq!(saved.query(), expected.query());
    assert_eq!(saved.definition().rows()[1].expression(), Some(text));

    // Native type zero still needs an explicit untyped-parameter contract.
    assert_eq!(
        reconstruct_saved_select(definition, Default::default(), &mut budget(), &|| false).err(),
        Some(E::Unsupported)
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn native_derived_outer_join_executes_through_existing_snapshot_contracts_without_writes() {
    let rows = [
        Fragment {
            name1: Some("KeyA"),
            ..column("d.Id")
        },
        Fragment {
            name1: Some("KeyB"),
            order: Some(&[0, 0, 0, 2]),
            ..column("i.Id")
        },
        derived(
            "SELECT Id FROM Items WHERE Id<3; -- retained metadata",
            Some("d"),
        ),
        source("Items", Some("i")),
        join(2, "d", "i", "d.Id=i.Id AND i.Id<>2"),
    ];
    for code in [1, 2, 5, 6] {
        let mut bytes = database_with_query_flags(&rows, 0);
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let before = bytes.clone();
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
        assert_eq!(
            prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            &[
                vec![Value::Int32(1), Value::Int32(1)],
                vec![Value::Int32(2), Value::Null]
            ]
        );
        drop(source);
        assert_eq!(bytes, before);
    }
}

#[test]
fn derived_guards_charge_syntax_work_and_memory_and_check_cancellation_before_publication() {
    let rows = [
        column("1"),
        derived("SELECT Id FROM Items WHERE Id>1; -- ; stored", Some("d")),
    ];
    let calls = Cell::new(0);
    reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let checks = Cell::new(0);
        assert_eq!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut budget(),
                &|| {
                    checks.set(checks.get() + 1);
                    checks.get() == stop
                }
            )
            .err(),
            Some(E::Cancelled)
        );
    }
    for mut allowance in [
        ResourceBudget::new(0, 100, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 100, 0),
    ] {
        assert!(matches!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut allowance,
                &|| false
            ),
            Err(E::Limit(_))
        ));
    }
    assert!(matches!(
        reconstruct_saved_select(
            definition(&rows, 0),
            ParseLimits {
                max_bytes: 32,
                ..Default::default()
            },
            &mut budget(),
            &|| false
        ),
        Err(E::SyntaxLimit)
    ));
}
