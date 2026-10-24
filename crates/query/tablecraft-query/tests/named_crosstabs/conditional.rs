use super::*;

#[test]
fn constant_choices_compose_through_named_selects_and_unions_without_discovery() {
    for headings in [
        "IIf(True,2,99),Choose(1,1,99),Switch(False,99,True,4)",
        "Nz(Null,2),Nz(1,99),IIf(Null,99,4)",
        "IIf(1=1,Nz(Null,2),3),Choose(1.5,99,1),Nz(Switch(False,99),4)",
    ] {
        let conditional = FIXED.replace("IN (2,1,4)", &format!("IN ({headings})"));
        let source = Views::new(&[
            ("Piv", &conditional),
            ("Wrapped", "SELECT Region,[2],[1],[4] FROM Piv"),
            (
                "Combined",
                "SELECT * FROM Wrapped UNION ALL SELECT * FROM Wrapped",
            ),
            ("Baseline", FIXED),
        ]);
        let parsed =
            parse_query("SELECT * FROM Combined ORDER BY Region", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0, "{headings}");

        let actual = plan.execute(&source, &[], Default::default()).unwrap();
        let expected = source
            .run(
                "SELECT Region,[2],[1],[4] FROM Baseline UNION ALL SELECT Region,[2],[1],[4] FROM Baseline ORDER BY Region",
                &[],
            )
            .unwrap();
        assert_eq!(actual.columns(), expected.columns(), "{headings}");
        assert_eq!(actual.output_types(), expected.output_types(), "{headings}");
        assert_eq!(actual.rows(), expected.rows(), "{headings}");
    }
}

#[test]
fn text_choices_preserve_names_and_omit_policy_dependent_condition_conversions() {
    let sql = concat!(
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region ",
        "PIVOT IIf(Quarter=1,'Sales.Q1','Sales!Q2') ",
        "IN (IIf(True,'Sales.Q1','unused'),",
        "Nz(Switch(False,'unused'),Choose(2,'unused','Sales!Q2')))"
    );
    let source = Views::new(&[("Piv", sql)]);
    let parsed = parse_query("SELECT * FROM Piv ORDER BY Region", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);

    // Metadata derives literal names without a comparer. Matching runtime Text
    // pivot values to those headings still requires the caller's explicit policy.
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.columns(), ["Region", "Sales.Q1", "Sales!Q2"]);
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), Value::Int64(3), Value::Int64(17)],
            vec![Value::Int32(2), Value::Null, Value::Int64(5)],
            vec![Value::Int32(3), Value::Null, Value::Int64(8)],
        ]
    );
}

#[test]
fn eager_unselected_errors_remain_visible_even_for_empty_sources() {
    for heading in [
        "IIf(True,2,1/0)",
        "IIf(False,1/0,2)",
        "Nz(2,1/0)",
        "Choose(1,2,1/0)",
        "Switch(True,2,False,1/0)",
    ] {
        let sql = FIXED.replace("IN (2,1,4)", &format!("IN ({heading})"));
        let mut source = Views::new(&[("Piv", &sql)]);
        let columns = source.data.columns("Sales").unwrap().to_vec();
        source.data = MemorySource::new(vec![
            MemoryTable::new("Sales".into(), columns, Vec::new()).unwrap(),
        ])
        .unwrap();
        let parsed = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{heading}"
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn conditional_constants_keep_heading_validation_and_contextual_refusal() {
    for (heading, error) in [
        ("IIf(True,2,3),2", QueryError::AmbiguousName),
        ("Nz(Null,'Region')", QueryError::AmbiguousName),
        ("IIf(True,Null,2)", QueryError::Unsupported),
        ("Choose(0,2)", QueryError::Unsupported),
        ("Switch(False,2)", QueryError::Unsupported),
        ("IIf('1',2,3)", QueryError::Unsupported),
        ("Choose('1',2)", QueryError::Unsupported),
        ("Switch('1',2)", QueryError::Unsupported),
        ("IIf(True,2,CLng(Rnd()))", QueryError::Unsupported),
        ("Nz(2,P)", QueryError::Unsupported),
        ("Choose(1,2,CLng('2'))", QueryError::Unsupported),
    ] {
        let sql = format!(
            "PARAMETERS P LONG; TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN ({heading})"
        );
        let source = Views::new(&[("Piv", &sql)]);
        let parsed = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(error),
            "{heading}"
        );
        assert_eq!(source.scans.get(), 0);
    }
}
