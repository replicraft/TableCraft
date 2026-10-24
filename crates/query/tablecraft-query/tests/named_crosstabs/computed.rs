use super::*;
use tablecraft_expressions::functions::{
    conversions::text::NumberFormat, dates::parsing::ReferenceYear,
};
use tablecraft_query::{PreparationOptions, prepare_with_options};

#[test]
fn computed_integer_headings_declare_schemas_without_scans_and_keep_empty_cells() {
    let sql = FIXED.replace("IN (2,1,4)", "IN (1+1,CLng(4/4),CInt(2^2))");
    let mut source = Views::new(&[("Piv", &sql)]);
    let parsed = parse_query("SELECT * FROM Piv ORDER BY Region", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(result.columns(), ["Region", "Total", "2", "1", "4"]);
    assert_eq!(
        result.rows(),
        [
            vec![
                Value::Int32(1),
                Value::Int64(20),
                Value::Int64(17),
                Value::Int64(3),
                Value::Null
            ],
            vec![
                Value::Int32(2),
                Value::Int64(5),
                Value::Null,
                Value::Null,
                Value::Null
            ],
            vec![
                Value::Int32(3),
                Value::Int64(8),
                Value::Null,
                Value::Null,
                Value::Null
            ],
        ]
    );
    assert_eq!(
        source
            .run("SELECT P.[2] FROM Piv AS P WHERE Region=1", &[])
            .unwrap()
            .rows(),
        [vec![Value::Int64(17)]]
    );
    let columns = source.data.columns("Sales").unwrap().to_vec();
    source.data = MemorySource::new(vec![
        MemoryTable::new("Sales".into(), columns, Vec::new()).unwrap(),
    ])
    .unwrap();
    let empty = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(empty.columns(), ["Region", "Total", "2", "1", "4"]);
    assert!(empty.rows().is_empty());
}

#[test]
fn computed_text_names_retain_punctuation_and_original_runtime_evaluation() {
    let sql = concat!(
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region ORDER BY Region ",
        "PIVOT IIf(Quarter=1,'Sales.Q1','Sales!Q2') IN ('Sales'&'.Q1','Sales'+'!Q2')"
    );
    let source = Views::new(&[("Piv", sql)]);
    let parsed = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
    let _ = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = source.run("SELECT * FROM Piv", &[]).unwrap();
    assert_eq!(result.columns(), ["Region", "Sales.Q1", "Sales!Q2"]);
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), Value::Int64(3), Value::Int64(17)],
            vec![Value::Int32(2), Value::Null, Value::Int64(5)],
            vec![Value::Int32(3), Value::Null, Value::Int64(8)],
        ]
    );
    assert_eq!(
        source
            .run("SELECT P.[Sales!Q2] FROM Piv AS P WHERE Region=1", &[])
            .unwrap()
            .rows(),
        [vec![Value::Int64(17)]]
    );
}

#[test]
fn pure_numeric_calls_and_conversions_do_not_capture_execution_settings() {
    let sql = FIXED.replace(
        "IN (2,1,4)",
        "IN (CVar(Abs(-2)),CByte(Sgn(9)),CLng(Sqr(16)))",
    );
    let source = Views::new(&[("Piv", &sql)]);
    let parsed = parse_query("SELECT * FROM Piv ORDER BY Region", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    for number_format in [None, Some(NumberFormat::DeDe)] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.columns(), ["Region", "Total", "2", "1", "4"]);
        assert_eq!(
            result.rows()[0],
            [
                Value::Int32(1),
                Value::Int64(20),
                Value::Int64(17),
                Value::Int64(3),
                Value::Null
            ]
        );
    }
}

#[test]
fn computed_schema_errors_limits_and_context_dependencies_fail_before_scans() {
    for (heading, error) in [
        ("1/0", QueryError::Evaluation(EvalError::DivisionByZero)),
        (
            "9223372036854775807+1",
            QueryError::Evaluation(EvalError::Overflow),
        ),
        ("1+1,2", QueryError::AmbiguousName),
        ("'Reg'&'ion'", QueryError::AmbiguousName),
        ("1.5+1", QueryError::Unsupported),
        ("CLng('2')", QueryError::Unsupported),
        ("CLng(Rnd())", QueryError::Unsupported),
        ("Year(Date())", QueryError::Unsupported),
        ("P+1", QueryError::Unsupported),
        ("CInt('a'='A')", QueryError::Unsupported),
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
    let sql = format!(
        "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN ('{}'&'{}')",
        "a".repeat(128),
        "b".repeat(128)
    );
    let source = Views::new(&[("Piv", &sql)]);
    let parsed = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn date_derived_integer_headings_capture_the_lexical_year_without_scanning() {
    let source = Views::new(&[(
        "Piv",
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region PIVOT CLng(#Mar 7#) IN (CLng(#Mar 7#))",
    )]);
    let parsed = parse_query("SELECT * FROM Piv ORDER BY Region", Default::default()).unwrap();
    let plan = prepare_with_options(
        parsed.query(),
        &source,
        PreparationOptions {
            reference_year: Some(ReferenceYear::new(2024).unwrap()),
        },
    )
    .unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(plan.output_types().unwrap().len(), 2);

    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                reference_year: Some(ReferenceYear::new(2030).unwrap()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), Value::Int64(20)],
            vec![Value::Int32(2), Value::Int64(5)],
            vec![Value::Int32(3), Value::Int64(8)],
        ]
    );
}

#[test]
fn runtime_domain_and_dynamic_text_dates_use_the_runtime_year_after_prepared_dates_are_captured() {
    let source = Views::new(&[]);
    let parsed = parse_query(
        "SELECT Year(#Mar 7#),DSum('Year(#Mar 7#)','Sales'),Eval('Year(#Mar 7#)')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare_with_options(
        parsed.query(),
        &source,
        PreparationOptions {
            reference_year: Some(ReferenceYear::new(2024).unwrap()),
        },
    )
    .unwrap();
    assert_eq!(source.scans.get(), 0);

    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                reference_year: Some(ReferenceYear::new(2030).unwrap()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int16(2024),
            Value::Int64(12180),
            Value::Int16(2030)
        ]]
    );
}
