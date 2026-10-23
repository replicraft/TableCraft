use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
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
fn cashflow_scalar_text_retains_shared_conversion_through_query_scopes() {
    for (text, numeric) in [
        (
            "NPV('0,25',Array(-100,0,125))",
            "NPV(0.25,Array(-100,0,125))",
        ),
        ("IRR(Array(-100,0,121),'0,1')", "IRR(Array(-100,0,121),0.1)"),
        (
            "MIRR(Array(-100,0,121),'0,0','0,0')",
            "MIRR(Array(-100,0,121),0,0)",
        ),
    ] {
        let expected = run(&format!("SELECT {numeric}"), None).unwrap();
        assert_eq!(
            run(&format!("SELECT {text}"), Some(NumberFormat::DeDe)).unwrap(),
            expected
        );
        assert_eq!(
            run(
                &format!("SELECT Q.N FROM (SELECT {text} AS N) AS Q"),
                Some(NumberFormat::DeDe)
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            run(&format!("SELECT {text}"), None),
            Err(EvalError::NumberFormatRequired.into())
        );
    }
    let parsed = parse_query(
        "PARAMETERS R TEXT; SELECT NPV(R,Array(-100,0,125))",
        Default::default(),
    )
    .unwrap();
    let source = MemorySource::default();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, text) in [(NumberFormat::EnUs, "0.25"), (NumberFormat::DeDe, "0,25")] {
        assert_eq!(
            plan.execute(
                &source,
                &[("R", Value::Text(text.into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            run("SELECT NPV(0.25,Array(-100,0,125))", None).unwrap()
        );
    }
}

#[test]
fn cashflow_text_does_not_change_array_admission_or_eager_failures() {
    for (sql, error) in [
        (
            "SELECT NPV('bad',Array(NULL))",
            EvalError::NumberFormatRequired,
        ),
        (
            "SELECT IRR(Array(NULL),'bad')",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT MIRR(Array(NULL),'bad','bad')",
            EvalError::InvalidNullConversion,
        ),
        ("SELECT NPV('bad',Array(1/0))", EvalError::DivisionByZero),
    ] {
        assert_eq!(run(sql, None), Err(error.into()), "{sql}");
    }
    assert_eq!(
        run(
            "SELECT NPV('0',Array('-100',121))",
            Some(NumberFormat::EnUs)
        ),
        Err(EvalError::TypeMismatch.into())
    );
}
