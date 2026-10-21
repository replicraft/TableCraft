use super::joins::{join, source};
use super::*;
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_query::prepare;
fn edge<'a>(
    kind: i16,
    left: &'a str,
    right: &'a str,
    on: &'a str,
    order: &'a [u8],
) -> Fragment<'a> {
    Fragment {
        order: Some(order),
        ..join(kind, left, right, on)
    }
}
#[test]
fn nested_and_disconnected_forests_preserve_orientation_root_order_and_inner_conditions() {
    let keys = [[0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 3], [0, 0, 0, 4]];
    for (edges, sql) in [
        (
            vec![(1, "a", "b", "a.ID=b.ID"), (2, "b", "c", "b.ID=c.ID")],
            "SELECT 1 FROM D AS d,(A AS a INNER JOIN B AS b ON a.ID=b.ID) LEFT JOIN C AS c ON b.ID=c.ID",
        ),
        (
            vec![
                (2, "a", "b", "a.ID=b.ID"),
                (2, "b", "c", "b.ID=c.ID"),
                (2, "b", "a", "b.ID<>2"),
            ],
            "SELECT 1 FROM D AS d,(A AS a LEFT JOIN B AS b ON (b.ID<>2) AND (a.ID=b.ID)) LEFT JOIN C AS c ON b.ID=c.ID",
        ),
        (
            vec![
                (2, "a", "b", "a.ID=b.ID"),
                (1, "a", "c", "a.ID=c.ID"),
                (1, "a", "b", "b.ID<>2"),
            ],
            "SELECT 1 FROM D AS d,(A AS a LEFT JOIN B AS b ON a.ID=b.ID) INNER JOIN C AS c ON (b.ID<>2) AND (a.ID=c.ID)",
        ),
        (
            vec![
                (3, "c", "d", "c.ID=d.ID"),
                (2, "b", "a", "a.ID=b.ID"),
                (1, "a", "d", "a.ID=d.ID"),
            ],
            "SELECT 1 FROM (B AS b LEFT JOIN A AS a ON a.ID=b.ID) INNER JOIN (C AS c RIGHT JOIN D AS d ON c.ID=d.ID) ON a.ID=d.ID",
        ),
    ] {
        let mut rows = vec![
            column("1"),
            source("A", Some("a")),
            source("B", Some("b")),
            source("C", Some("c")),
            source("D", Some("d")),
        ];
        rows.extend(
            edges
                .iter()
                .enumerate()
                .map(|(i, (kind, left, right, on))| edge(*kind, left, right, on, &keys[i])),
        );
        assert_eq!(
            compile(&rows).unwrap().query(),
            parse_query(sql, Default::default()).unwrap().query()
        );
    }
}
#[test]
fn source_and_condition_caps_ambiguity_conflicts_and_injection_are_bounded() {
    for count in [1, 2, 16, 17] {
        let names: Vec<_> = (0..count).map(|i| format!("T{i}")).collect();
        let mut rows = vec![column("1")];
        rows.extend(names.iter().map(|name| source(name, None)));
        let result = compile(&rows);
        if count <= 16 {
            let saved = result.unwrap();
            let tablecraft_sql::query::Statement::Read(read) = &saved.query().statement else {
                panic!()
            };
            let tablecraft_sql::query::ReadBody::Select(select) = &read.body else {
                panic!()
            };
            assert_eq!(select.from.len(), count);
        } else {
            assert_eq!(result.err(), Some(E::Unsupported));
        }
    }
    let key = [0, 0, 0, 2];
    for (left, right, kind, on) in [
        ("a", "missing", 1, "True"),
        ("a", "a", 1, "True"),
        ("a", "b", 3, "True"),
        ("a", "c", 0, "True"),
        ("a", "c", 1, "True) WHERE False"),
    ] {
        let rows = [
            column("1"),
            source("A", Some("a")),
            source("B", Some("b")),
            source("C", Some("c")),
            join(2, "a", "b", "True"),
            edge(kind, left, right, on, &key),
        ];
        assert!(compile(&rows).is_err());
    }
    let rows = [
        column("1"),
        source("A", Some("a")),
        source("B", Some("A")),
        source("C", Some("c")),
    ];
    assert_eq!(compile(&rows).err(), Some(E::InvalidMetadata));
}

#[test]
fn connected_forest_reaches_the_last_source_bit_and_full_condition_ceiling() {
    let names: Vec<_> = (0..16).map(|i| format!("T{i}")).collect();
    let orders: Vec<_> = (1_u8..=16).map(|i| [0, 0, 0, i]).collect();
    let mut rows = vec![column("1")];
    rows.extend(names.iter().map(|name| source(name, None)));
    let mut expected = String::from("SELECT 1 FROM T0");
    for i in 1..16 {
        rows.push(edge(1, &names[i - 1], &names[i], "True", &orders[i - 1]));
        expected.push_str(&format!(" INNER JOIN T{i} ON True"));
    }
    // The sixteenth edge attaches to the root spanning the first and last
    // source bits, rather than overflowing the membership set or making a cycle.
    rows.push(edge(1, &names[15], &names[0], "False", &orders[15]));
    expected.truncate(expected.len() - "True".len());
    expected.push_str("(False) AND (True)");
    assert_eq!(
        compile(&rows).unwrap().query(),
        parse_query(&expected, Default::default()).unwrap().query()
    );
}
#[test]
fn native_outer_forests_attach_late_predicates_before_null_extension_without_writes() {
    let keys = [[0, 0, 0, 2], [0, 0, 0, 3]];
    let rows = [
        Fragment {
            name1: Some("KeyA"),
            ..column("a.Id")
        },
        Fragment {
            name1: Some("KeyB"),
            order: Some(&[0, 0, 0, 2]),
            ..column("b.Id")
        },
        Fragment {
            name1: Some("KeyC"),
            order: Some(&[0, 0, 0, 3]),
            ..column("c.Id")
        },
        source("Items", Some("a")),
        source("Items", Some("b")),
        source("Items", Some("c")),
        join(2, "a", "b", "a.Id=b.Id"),
        edge(2, "b", "c", "b.Id=c.Id", &keys[0]),
        edge(2, "b", "a", "b.Id<>2", &keys[1]),
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
            [
                vec![Value::Int32(1), Value::Int32(1), Value::Int32(1)],
                vec![Value::Int32(2), Value::Null, Value::Null],
                vec![Value::Int32(3), Value::Int32(3), Value::Int32(3)]
            ]
        );
        drop(source);
        assert_eq!(bytes, before);
    }
}

#[test]
fn forest_construction_and_publication_honor_work_memory_syntax_and_cancellation() {
    let rows = [
        column("1"),
        source("A", Some("a")),
        source("B", Some("b")),
        source("C", Some("c")),
        join(2, "a", "b", "a.ID=b.ID"),
        edge(1, "b", "c", "b.ID=c.ID", &[0, 0, 0, 2]),
        edge(2, "b", "a", "b.ID<>2", &[0, 0, 0, 3]),
    ];
    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    let total = checks.get();
    // Every construction and final publication checkpoint must refuse without
    // returning an owned model, including predicates attached to an inner node.
    for stop in 1..=total {
        let calls = Cell::new(0);
        assert_eq!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
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
    let limits = ParseLimits {
        max_depth: 1,
        ..Default::default()
    };
    assert!(
        reconstruct_saved_select(definition(&rows, 0), limits, &mut budget(), &|| false).is_err()
    );
}
