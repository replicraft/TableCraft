use super::*;

#[test]
fn ordinal_and_ascii_equivalence_keep_independent_counts_and_first_spelling() {
    let source = Input::new(
        &[("Label", ScalarType::Text)],
        [
            Some("B"),
            Some("a"),
            Some("B"),
            None,
            Some("a"),
            Some("b"),
            None,
        ]
        .into_iter()
        .map(|text| vec![text.map_or(Value::Null, |text| Value::Text(text.into()))])
        .collect(),
    );
    for declared_integral in [false, true] {
        for (order, classes) in [
            (
                &ORDINAL as &dyn Collation,
                vec![(Some("B"), 2), (Some("a"), 2), (None, 2), (Some("b"), 1)],
            ),
            (&FOLDED, vec![(Some("B"), 3), (Some("a"), 2), (None, 2)]),
        ] {
            let rows = stream(
                "SELECT Label,Count(*) FROM T GROUP BY Label",
                &source,
                declared_integral,
                ExecutionOptions {
                    collation: Some(order),
                    declared_integral,
                    ..Default::default()
                },
            )
            .unwrap();
            let expected: Vec<_> = classes
                .into_iter()
                .map(|(key, count)| {
                    vec![
                        key.map_or(Value::Null, |text| Value::Text(text.into())),
                        Value::Int64(count),
                    ]
                })
                .collect();
            assert_eq!(rows, expected);
            let with_null_component = stream(
                "SELECT Label,NULL,Count(*) FROM T GROUP BY Label,NULL",
                &source,
                declared_integral,
                ExecutionOptions {
                    collation: Some(order),
                    declared_integral,
                    ..Default::default()
                },
            )
            .unwrap();
            let expected: Vec<_> = expected
                .iter()
                .map(|row| vec![row[0].clone(), Value::Null, row[1].clone()])
                .collect();
            assert_eq!(with_null_component, expected);
        }
    }
}

#[test]
fn late_final_provider_repartitions_all_earlier_string_keys_before_aggregates() {
    let mut source = Input::new(
        &[
            ("Id", ScalarType::Int32),
            ("A", ScalarType::Text),
            ("B", ScalarType::Text),
        ],
        (1..=3)
            .map(|id| {
                vec![
                    Value::Int32(id),
                    Value::Text("unused".into()),
                    Value::Text("3".into()),
                ]
            })
            .collect(),
    );
    source.policies = vec![
        None,
        Some(TextCollationId::from_host(1)),
        Some(TextCollationId::from_host(2)),
    ];
    let key = "IIf(Id=3,B,Id)&IIf(True,0,A)";
    let sql = format!("SELECT {key},Count(*),Sum(Id) FROM T GROUP BY {key}");
    for (root, final_order, expected) in [
        (
            &ORDINAL as &dyn Collation,
            &ALL_EQUAL as &'static dyn Collation,
            vec![vec![
                Value::Text("10".into()),
                Value::Int64(3),
                Value::Int64(6),
            ]],
        ),
        (
            &ALL_EQUAL,
            &ORDINAL,
            vec![
                vec![Value::Text("10".into()), Value::Int64(1), Value::Int64(1)],
                vec![Value::Text("20".into()), Value::Int64(1), Value::Int64(2)],
                vec![Value::Text("30".into()), Value::Int64(1), Value::Int64(3)],
            ],
        ),
    ] {
        let resolver = Resolver {
            source: &source,
            order: final_order,
            calls: Cell::new(0),
        };
        let rows = stream(
            &sql,
            &source,
            false,
            ExecutionOptions {
                collation: Some(root),
                collation_resolver: Some(&resolver),
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(rows, expected);
        assert!(resolver.calls.get() > 0);
    }
}

fn fixed(kind: ScalarType, value: i32, variant: bool) -> Value {
    match kind {
        ScalarType::Boolean => Value::Boolean(value != 0),
        ScalarType::Byte => Value::Byte(value as u8),
        ScalarType::Int16 => Value::Int16(value as i16),
        ScalarType::Int32 => Value::Int32(value),
        ScalarType::Int64 => Value::Int64(i64::from(value)),
        ScalarType::Currency => Value::Currency(Currency::from_scaled(i64::from(value))),
        ScalarType::Decimal => Value::Decimal(
            Decimal::new(
                i128::from(value) * if variant { 100 } else { 1 },
                if variant { 2 } else { 0 },
            )
            .unwrap(),
        ),
        _ => panic!("explicit fixed-key family required"),
    }
}

#[test]
fn text_tuples_admit_every_fixed_family_and_keep_first_representational_key() {
    for kind in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
        ScalarType::Currency,
        ScalarType::Decimal,
    ] {
        let source = Input::new(
            &[("Label", ScalarType::Text), ("K", kind)],
            [
                ("b", Some(1)),
                ("A", Some(0)),
                ("B", Some(1)),
                ("a", Some(0)),
                ("a", None),
                ("A", None),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (label, value))| {
                vec![
                    Value::Text(label.into()),
                    value.map_or(Value::Null, |value| fixed(kind, value, index < 2)),
                ]
            })
            .collect(),
        );
        let expected = vec![
            vec![
                Value::Text("b".into()),
                fixed(kind, 1, true),
                Value::Int64(2),
            ],
            vec![
                Value::Text("A".into()),
                fixed(kind, 0, true),
                Value::Int64(2),
            ],
            vec![Value::Text("a".into()), Value::Null, Value::Int64(2)],
        ];
        for prepare_declared in [false, true] {
            for declared_integral in [false, true] {
                let actual = stream(
                    "SELECT Label,K,Count(*) FROM T GROUP BY Label,K",
                    &source,
                    prepare_declared,
                    ExecutionOptions {
                        collation: Some(&FOLDED),
                        declared_integral,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(actual, expected, "{kind:?}");
            }
        }
    }
}

#[test]
fn null_text_and_empty_spelling_stay_distinct_under_paired_derived_alias_metadata() {
    let source = Input::new(
        &[("Label", ScalarType::Text)],
        [None, Some(""), Some("NULL"), None, Some(""), Some("null")]
            .into_iter()
            .map(|text| vec![text.map_or(Value::Null, |text| Value::Text(text.into()))])
            .collect(),
    );
    for prepare_declared in [false, true] {
        for declared_integral in [false, true] {
            let actual = stream(
                "SELECT K,VarType(K),Count(*) FROM (SELECT Label AS K FROM T) Q GROUP BY K",
                &source,
                prepare_declared,
                ExecutionOptions {
                    collation: Some(&FOLDED),
                    declared_integral,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                actual,
                vec![
                    vec![Value::Null, Value::Int16(1), Value::Int64(2)],
                    vec![Value::Text("".into()), Value::Int16(8), Value::Int64(2)],
                    vec![Value::Text("NULL".into()), Value::Int16(8), Value::Int64(2)],
                ]
            );
        }
    }
}
