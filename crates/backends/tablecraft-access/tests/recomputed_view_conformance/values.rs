use super::*;

#[test]
fn widened_results_never_coerce_back_to_the_stored_int32_declaration() {
    for profile in [5, 6] {
        let mut bytes = fixtures::database(&[("Wide", "[Id]+[Id]")], &[i32::MAX]);
        bytes[20] = profile;
        let source = native(bytes);
        let views = RecomputedNativeSource::new(
            &source,
            &[CalculationView::new("Fresh", "Items", &["Wide"])],
            ParseLimits::default(),
        )
        .unwrap();
        let syntax = parse_query("SELECT Wide FROM Fresh", ParseLimits::default()).unwrap();
        let plan = prepare(syntax.query(), &views).unwrap();

        assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
        let result = plan.execute(&views, &[], options(false)).unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Double(Double::from_float(4_294_967_294.0))]]
        );
        assert_eq!(result.output_types(), [OutputType::Dynamic]);
        assert!(matches!(
            plan.execute(&views, &[], options(true)),
            Err(QueryError::Evaluation(EvalError::Overflow))
        ));
    }
}

#[test]
fn boolean_text_and_null_results_keep_natural_descriptors_and_cells() {
    let source = native(fixtures::database(
        &[
            ("Flag", "[Id]>1"),
            ("Label", "IIf([Id]>1,'large','small')"),
            ("Absent", "Null"),
        ],
        &[1, 2, 3],
    ));
    let views = RecomputedNativeSource::new(
        &source,
        &[CalculationView::new(
            "Fresh",
            "Items",
            &["Id", "Flag", "Label", "Absent"],
        )],
        ParseLimits::default(),
    )
    .unwrap();
    let syntax = parse_query(
        "SELECT Flag,Label,Absent FROM Fresh ORDER BY Id",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &views).unwrap();
    let types = plan.output_types().unwrap();

    assert_eq!(scalar_type(types[0]), Some(ScalarType::Boolean));
    assert_eq!(scalar_type(types[1]), Some(ScalarType::Text));
    assert_eq!(types[2], OutputType::Null);
    for declared in [false, true] {
        let actual = plan.execute(&views, &[], options(declared)).unwrap();
        let expected = (1..=3)
            .map(|id| {
                vec![
                    Value::Boolean(id > 1),
                    Value::Text(if id > 1 { "large" } else { "small" }.into()),
                    Value::Null,
                ]
            })
            .collect::<Vec<_>>();
        assert_eq!(actual.rows(), expected);
    }
}

#[test]
fn transitive_values_compose_with_derived_join_group_and_domain_queries() {
    let source = native(fixtures::database(
        &[("Twice", "[Id]+[Id]"), ("Total", "[Twice]+[Id]")],
        &[1, 2, 3],
    ));
    let views = RecomputedNativeSource::new(
        &source,
        &[CalculationView::new("Fresh", "Items", &["Id", "Total"])],
        ParseLimits::default(),
    )
    .unwrap();

    for declared in [false, true] {
        let expected = (1..=3)
            .map(|id| vec![Value::Int32(id), Value::Int32(id * 3)])
            .collect::<Vec<_>>();
        for sql in [
            "SELECT Id,Total FROM Fresh ORDER BY Id",
            "SELECT Q.Id,Q.Total FROM (SELECT Id,Total FROM Fresh) Q ORDER BY Q.Id",
            "SELECT F.Id,F.Total FROM Fresh F INNER JOIN Items I ON F.Id=I.Id ORDER BY F.Id",
        ] {
            assert_eq!(rows(&views, sql, declared).unwrap(), expected, "{sql}");
        }
        let grouped = rows(
            &views,
            "SELECT Total,Count(*) FROM Fresh GROUP BY Total ORDER BY Total",
            declared,
        )
        .unwrap();
        let expected_groups = (1..=3)
            .map(|id| vec![Value::Int32(id * 3), Value::Int64(1)])
            .collect::<Vec<_>>();
        assert_eq!(grouped, expected_groups);

        let domain = rows(
            &views,
            "SELECT DLookup('Total','Fresh','Id=2') FROM Items WHERE Id=1",
            declared,
        )
        .unwrap();
        assert_eq!(domain, [vec![Value::Int32(6)]]);
    }
}

#[test]
fn union_promotes_natural_boolean_to_integer_without_using_cached_result_types() {
    let source = native(fixtures::database(&[("Flag", "[Id]>1")], &[1, 2]));
    let views = RecomputedNativeSource::new(
        &source,
        &[CalculationView::new("Fresh", "Items", &["Flag"])],
        ParseLimits::default(),
    )
    .unwrap();

    for declared in [false, true] {
        let actual = rows(
            &views,
            "SELECT Flag AS V FROM Fresh UNION ALL SELECT CLng(7) FROM Items WHERE Id=1",
            declared,
        )
        .unwrap();
        assert_eq!(
            actual,
            [
                vec![Value::Int32(0)],
                vec![Value::Int32(-1)],
                vec![Value::Int32(7)]
            ]
        );
    }
}
