use super::*;

#[test]
fn projecting_required_text_policies_never_activates_a_comparison_provider() {
    let mut source = Tables::new(&[]);
    source.data = MemorySource::new(
        ["A", "B"]
            .into_iter()
            .map(|name| {
                MemoryTable::new(
                    name.into(),
                    vec![Column::new("N".into(), ScalarType::Text, false).unwrap()],
                    vec![vec![Value::Text(name.into())]],
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    source.policies = vec![Some(TextCollationId::from_host(987))];
    let rows = stream(
        "SELECT N FROM A UNION ALL SELECT N FROM B",
        &source,
        Default::default(),
    );
    assert_eq!(
        rows,
        vec![vec![Value::Text("A".into())], vec![Value::Text("B".into())]]
    );
}

#[test]
fn whole_union_types_promote_earlier_rows_before_publication() {
    for declared in [false, true] {
        for (left, right, expected) in [
            ("CInt(1)", "CLng(2)", vec![Value::Int32(1), Value::Int32(2)]),
            (
                "CDec(1.25)",
                "CLng(2)",
                vec![
                    Value::Decimal(Decimal::new(125, 2).unwrap()),
                    Value::Decimal(Decimal::new(2, 0).unwrap()),
                ],
            ),
            (
                "CSng(1.25)",
                "CDbl(2.5)",
                vec![
                    Value::Double(Double::from_float(1.25)),
                    Value::Double(Double::from_float(2.5)),
                ],
            ),
            ("NULL", "CLng(2)", vec![Value::Null, Value::Int32(2)]),
        ] {
            let source = Tables::new(&[1]);
            let sql = format!("SELECT {left} AS V FROM A UNION ALL SELECT {right} FROM B");
            let rows = stream(
                &sql,
                &source,
                ExecutionOptions {
                    declared_integral: declared,
                    ..Default::default()
                },
            );
            assert_eq!(
                rows,
                expected
                    .into_iter()
                    .map(|value| vec![value])
                    .collect::<Vec<_>>()
            );
            assert_eq!(*source.scans.borrow(), ["A", "B"]);
        }
    }
}

#[test]
fn captured_paired_descriptors_select_the_execution_profile() {
    let source = Tables::new(&[1]);
    let sql = "SELECT CDec(3)/CDbl(2) AS V FROM A UNION ALL SELECT CLng(2) FROM B";
    for declared in [false, true] {
        let rows = stream(
            sql,
            &source,
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        );
        let expected = if declared {
            vec![
                vec![Value::Double(Double::from_float(1.5))],
                vec![Value::Double(Double::from_float(2.0))],
            ]
        } else {
            vec![
                vec![Value::Decimal(Decimal::new(15, 1).unwrap())],
                vec![Value::Decimal(Decimal::new(2, 0).unwrap())],
            ]
        };
        assert_eq!(rows.len(), expected.len());

        if declared {
            assert_eq!(rows, expected);
        } else {
            for (row, coefficient) in rows.iter().zip([15, 20]) {
                let Value::Decimal(value) = row[0] else {
                    panic!("default profile must retain Decimal");
                };
                assert_eq!(value.rescale_exact(1).unwrap().coefficient(), coefficient);
            }
        }
    }
}

#[test]
fn text_promotion_uses_explicit_format_and_strict_raw_publication() {
    let source = Tables::new(&[1]);
    for (format, spelling) in [
        (NumberFormat::Invariant, "1.25"),
        (NumberFormat::DeDe, "1,25"),
    ] {
        let rows = stream(
            "SELECT CDec(1.25) AS V FROM A UNION ALL SELECT LeftB('ok',4) FROM B",
            &source,
            ExecutionOptions {
                number_format: Some(format),
                ..Default::default()
            },
        );
        assert_eq!(
            rows,
            vec![
                vec![Value::Text(spelling.into())],
                vec![Value::Text("ok".into())]
            ]
        );
    }
}
