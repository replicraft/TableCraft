use super::*;

pub(super) fn source<'a>(table: &'a str, alias: Option<&'a str>) -> Fragment<'a> {
    Fragment {
        name1: Some(table),
        name2: alias,
        ..row(5)
    }
}
pub(super) fn join<'a>(
    kind: i16,
    left: &'a str,
    right: &'a str,
    condition: &'a str,
) -> Fragment<'a> {
    Fragment {
        flag: Some(kind),
        name1: Some(left),
        name2: Some(right),
        expression: Some(condition),
        ..row(7)
    }
}
#[test]
fn join_kind_and_orientation_follow_alias_endpoints_without_binding_sources() {
    for (flag, keyword) in [(1, "INNER"), (2, "LEFT"), (3, "RIGHT")] {
        for reverse_storage in [false, true] {
            for reverse_endpoints in [false, true] {
                let first = source("Left Table", Some("a"));
                let second = source("Right Table", Some("b"));
                let (first, second) = if reverse_storage {
                    (second, first)
                } else {
                    (first, second)
                };
                let (left, right, names) = if reverse_endpoints {
                    ("B", "a", ("Right Table", "b", "Left Table", "a"))
                } else {
                    ("A", "b", ("Left Table", "a", "Right Table", "b"))
                };
                let result = compile(&[
                    column("[a].[ID]"),
                    first,
                    second,
                    join(flag, left, right, "[a].[ID] = [b].[ID]"),
                ])
                .unwrap();
                let expected = parse_query(&format!("SELECT [a].[ID] FROM [{}] AS [{}] {keyword} JOIN [{}] AS [{}] ON [a].[ID] = [b].[ID]", names.0, names.1, names.2, names.3), Default::default()).unwrap();
                assert_eq!(result.query(), expected.query());
                assert_eq!(result.definition().rows().len(), 4);
            }
        }
    }
    let self_join = compile(&[
        column("[a].[ID]"),
        source("Items", Some("a")),
        source("Items", Some("b")),
        join(2, "a", "b", "[a].[ID] = [b].[ID]"),
    ])
    .unwrap();
    assert_eq!(
        self_join.query(),
        parse_query(
            "SELECT [a].[ID] FROM Items AS a LEFT JOIN Items AS b ON a.ID=b.ID",
            Default::default()
        )
        .unwrap()
        .query()
    );
    let unicode = compile(&[
        column("1"),
        source("Ordres 🦀", Some("a")),
        source("Éléments", Some("b")),
        join(1, "a", "b", "True"),
    ])
    .unwrap();
    assert_eq!(
        unicode.query(),
        parse_query(
            "SELECT 1 FROM [Ordres 🦀] AS a INNER JOIN [Éléments] AS b ON True",
            Default::default()
        )
        .unwrap()
        .query()
    );
}

#[test]
fn join_endpoints_require_declared_unambiguous_names_and_known_kinds() {
    for kind in [0, 4, -1, i16::MAX] {
        assert_eq!(
            compile(&[
                column("1"),
                source("L", None),
                source("R", None),
                join(kind, "L", "R", "True")
            ])
            .err(),
            Some(E::Unsupported)
        );
    }
    for (left, right) in [("missing", "R"), ("L", "missing"), ("L", "l"), ("", "R")] {
        assert_eq!(
            compile(&[
                column("1"),
                source("L", None),
                source("R", None),
                join(1, left, right, "True")
            ])
            .err(),
            Some(E::InvalidMetadata)
        );
    }
    assert_eq!(
        compile(&[
            column("1"),
            source("L", None),
            source("l", None),
            join(1, "L", "l", "True")
        ])
        .err(),
        Some(E::InvalidMetadata)
    );
    assert_eq!(
        compile(&[
            column("1"),
            source("L", Some("a")),
            source("R", Some("b")),
            join(1, "L", "R", "True")
        ])
        .err(),
        Some(E::InvalidMetadata)
    );
    assert_eq!(
        compile(&[column("1"), source("L", None), source("R", None)])
            .unwrap()
            .query(),
        parse_query("SELECT 1 FROM L,R", Default::default())
            .unwrap()
            .query()
    );
    assert_eq!(
        compile(&[column("1"), source("L", None), join(1, "L", "R", "True")]).err(),
        Some(E::InvalidMetadata)
    );
    assert_eq!(
        compile(&[column("1"), join(1, "L", "R", "True")]).err(),
        Some(E::InvalidMetadata)
    );
    assert_eq!(
        compile(&[
            column("1"),
            source("L", None),
            source("R", None),
            source("Third", None),
            join(1, "L", "R", "True")
        ])
        .unwrap()
        .query(),
        parse_query(
            "SELECT 1 FROM Third,L INNER JOIN R ON True",
            Default::default()
        )
        .unwrap()
        .query()
    );
    assert_eq!(
        compile(&[
            column("1"),
            source("L", None),
            source("R", None),
            join(1, "L", "R", "True"),
            join(1, "L", "R", "False")
        ])
        .unwrap()
        .query(),
        parse_query(
            "SELECT 1 FROM L INNER JOIN R ON (False) AND (True)",
            Default::default()
        )
        .unwrap()
        .query()
    );
    assert_eq!(
        compile(&[
            column("1"),
            source("É", None),
            source("R", None),
            join(1, "É", "R", "True")
        ])
        .err(),
        Some(E::Unsupported)
    );
    for field in 0..4 {
        let mut bad = join(1, "L", "R", "True");
        match field {
            0 => bad.flag = None,
            1 => bad.name1 = None,
            2 => bad.name2 = None,
            _ => bad.expression = None,
        }
        assert_eq!(
            compile(&[column("1"), source("L", None), source("R", None), bad]).err(),
            Some(E::InvalidMetadata)
        );
    }
}

#[test]
fn join_conditions_remain_one_owned_expression_and_fail_without_a_partial_model() {
    for condition in [
        "True; SELECT 2",
        "True, False",
        "True) WHERE 2 IN (2 --",
        "True) OR False OR (True",
        "True) INNER JOIN Injected ON (True",
    ] {
        assert!(matches!(
            compile(&[
                column("1"),
                source("L", None),
                source("R", None),
                join(1, "L", "R", condition)
            ]),
            Err(E::Parse(_))
        ));
    }
    let fragments = [
        column("1"),
        source("L", None),
        source("R", None),
        join(1, "L", "R", "NeverRunMe([L].[ID], [R].[ID]) -- comment"),
    ];
    let result = compile(&fragments).unwrap();
    assert_eq!(
        result.query(),
        parse_query(
            "SELECT 1 FROM L INNER JOIN R ON NeverRunMe(L.ID,R.ID)",
            Default::default()
        )
        .unwrap()
        .query()
    );
    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&fragments, 0),
        Default::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    let final_check = checks.get();
    checks.set(0);
    assert_eq!(
        reconstruct_saved_select(
            definition(&fragments, 0),
            Default::default(),
            &mut budget(),
            &|| {
                checks.set(checks.get() + 1);
                checks.get() == final_check
            }
        )
        .err(),
        Some(E::Cancelled)
    );
    let mut limited = ResourceBudget::new(256, 100, 1_000_000);
    assert!(matches!(
        reconstruct_saved_select(
            definition(&fragments, 0),
            Default::default(),
            &mut limited,
            &|| false
        ),
        Err(E::Limit(_))
    ));
}

#[test]
fn compound_join_conditions_preserve_reference_prepend_order_for_each_join_kind() {
    let expressions = [
        "MarkFirst([L].[ID], [R].[ID])",
        "MarkSecond([L].[ID], [R].[ID])",
        "MarkThird([L].[ID], [R].[ID])",
    ];
    let keys = [[0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 3]];
    for (kind, keyword) in [(1, "INNER"), (2, "LEFT"), (3, "RIGHT")] {
        for count in [2, 3] {
            let mut fragments = vec![column("1"), source("L", None), source("R", None)];
            for (expression, key) in expressions.iter().zip(&keys).take(count) {
                fragments.push(Fragment {
                    order: Some(key),
                    ..join(kind, "l", "R", expression)
                });
            }
            let actual = compile(&fragments).unwrap();
            let expected_condition = expressions
                .iter()
                .take(count)
                .rev()
                .map(|s| format!("({s})"))
                .collect::<Vec<_>>()
                .join(" AND ");
            let expected = parse_query(
                &format!("SELECT 1 FROM L {keyword} JOIN R ON {expected_condition}"),
                Default::default(),
            )
            .unwrap();
            assert_eq!(actual.query(), expected.query());
        }
    }
}

#[test]
fn compound_join_profile_rejects_mixed_kinds_endpoints_invalid_fragments_and_unbounded_work() {
    let key = [0, 0, 0, 2];
    for case in 0..7 {
        let mut second = Fragment {
            order: Some(&key),
            ..join(2, "L", "R", "True")
        };
        match case {
            0 => second.flag = Some(3),
            1 => second.name1 = Some("R"),
            2 => second.name1 = Some("Missing"),
            3 => second.flag = None,
            4 => second.name2 = None,
            5 => second.expression = Some("True) OR (False"),
            _ => second.extra = Some(0),
        }
        assert!(
            compile(&[
                column("1"),
                source("L", None),
                source("R", None),
                join(2, "L", "R", "True"),
                second
            ])
            .is_err()
        );
    }
    let keys: Vec<_> = (1..=17_u32).map(u32::to_be_bytes).collect();
    for count in [16, 17] {
        let mut fragments = vec![column("1"), source("L", None), source("R", None)];
        fragments.extend(keys.iter().take(count).map(|key| Fragment {
            order: Some(key),
            ..join(1, "L", "R", "NeverRunMe(1)")
        }));
        let actual = compile(&fragments);
        if count == 16 {
            assert!(actual.is_ok());
        } else {
            assert_eq!(actual.err(), Some(E::SyntaxLimit));
        }
    }
    let fragments = [
        column("1"),
        source("L", None),
        source("R", None),
        join(1, "L", "R", "First()"),
        Fragment {
            order: Some(&key),
            ..join(1, "L", "R", "Second()")
        },
    ];
    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&fragments, 0),
        Default::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    let final_check = checks.get();
    checks.set(0);
    assert_eq!(
        reconstruct_saved_select(
            definition(&fragments, 0),
            Default::default(),
            &mut budget(),
            &|| {
                checks.set(checks.get() + 1);
                checks.get() == final_check
            }
        )
        .err(),
        Some(E::Cancelled)
    );
    assert!(matches!(
        reconstruct_saved_select(
            definition(&fragments, 0),
            Default::default(),
            &mut ResourceBudget::new(1024, 100, 1_000_000),
            &|| false
        ),
        Err(E::Limit(_))
    ));
}
