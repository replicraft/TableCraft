use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, OutputType, QueryError, prepare, source::MemorySource};
use tablecraft_schema::ScalarType;
use tablecraft_sql::parse_query;

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn numeric_text_math_composes_through_sources_parameters_and_round_controls() {
    let sql = "SELECT Abs('-2,5'),Int('-2,5'),Fix('-2,5'),Sgn('-2,5'),Sqr('2,25'),RGB('2,5','1,5','3,5'),QBColor('2,5'),Round(CDec(2.675),'2')";
    let numeric_sql = "SELECT Abs(CDbl(-2.5)),Int(CDbl(-2.5)),Fix(CDbl(-2.5)),Sgn(-2.5),Sqr(2.25),RGB(2.5,1.5,3.5),QBColor(2.5),Round(CDec(2.675),2)";
    assert_eq!(
        run(sql, Some(NumberFormat::DeDe)).unwrap(),
        run(numeric_sql, None).unwrap()
    );
    assert_eq!(
        run(
            "SELECT Abs(Q.N) FROM (SELECT '-2,5' AS N) AS Q",
            Some(NumberFormat::DeDe)
        )
        .unwrap(),
        run("SELECT Abs(CDbl(-2.5))", None).unwrap()
    );
    let source = MemorySource::default();
    let parsed = parse_query("PARAMETERS P TEXT; SELECT Round(P,0)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, value) in [(NumberFormat::EnUs, "2.5"), (NumberFormat::DeDe, "2,5")] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text(value.into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            run("SELECT Round(CDbl(2.5),0)", None).unwrap()
        );
    }
}

#[test]
fn math_text_keeps_explicit_policy_domains_and_eager_argument_failures() {
    assert_eq!(
        run("SELECT Abs('1')", None),
        Err(EvalError::NumberFormatRequired.into())
    );
    for (sql, error) in [
        ("SELECT Round('bad',29)", EvalError::Domain),
        ("SELECT RGB(-1,'bad',0)", EvalError::Domain),
        ("SELECT Round('bad',1/0)", EvalError::DivisionByZero),
        ("SELECT RGB(-1,'bad',1/0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(error.into()),
            "{sql}"
        );
    }
}

#[test]
fn math_text_output_metadata_is_double_without_converting_or_reading_inputs() {
    let source = MemorySource::default();
    for expression in [
        "Abs('bad')",
        "Int('bad')",
        "Fix('bad')",
        "Round('bad','bad')",
    ] {
        let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let output_types = plan.output_types().unwrap();
        let OutputType::Scalar { data_type, .. } = &output_types[0] else {
            panic!("expected scalar metadata for {expression}");
        };
        assert_eq!(data_type.scalar_type(), ScalarType::Double, "{expression}");
    }
    assert_eq!(
        run(
            "SELECT Q.N FROM (SELECT Abs('1') AS N UNION ALL SELECT CInt(2)) AS Q",
            Some(NumberFormat::EnUs)
        )
        .unwrap(),
        run("SELECT CDbl(1) UNION ALL SELECT CDbl(2)", None).unwrap()
    );
}
