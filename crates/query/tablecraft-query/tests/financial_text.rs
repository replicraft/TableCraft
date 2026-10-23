use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(expression: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
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
fn every_scalar_financial_function_accepts_explicit_numeric_text_without_changing_results() {
    for (text, numeric) in [
        ("Pmt('0','10','1000')", "Pmt(0,10,1000)"),
        ("PV('0','10','-100')", "PV(0,10,-100)"),
        ("FV('0','10','-100')", "FV(0,10,-100)"),
        ("NPer('0','-100','1000')", "NPer(0,-100,1000)"),
        ("IPmt('0','1','10','1000')", "IPmt(0,1,10,1000)"),
        ("PPmt('0','1','10','1000')", "PPmt(0,1,10,1000)"),
        ("SLN('1000','0','10')", "SLN(1000,0,10)"),
        ("SYD('1100','0','10','1')", "SYD(1100,0,10,1)"),
        ("DDB('1000','0','10','1')", "DDB(1000,0,10,1)"),
        ("Rate('10','-100','1000',,,'0')", "Rate(10,-100,1000,,,0)"),
        (
            "EuroConvert('1,5','EUR','EUR','1','3,5')",
            "EuroConvert(1.5,'EUR','EUR',True,4)",
        ),
    ] {
        assert_eq!(
            run(text, Some(NumberFormat::DeDe)).unwrap(),
            run(numeric, None).unwrap(),
            "{text}"
        );
        assert_eq!(
            run(text, None),
            Err(EvalError::NumberFormatRequired.into()),
            "{text}"
        );
    }
    assert_eq!(
        run(
            "(SELECT SLN(Q.Cost,'0','10')) FROM (SELECT '1000,0' AS Cost) AS Q",
            Some(NumberFormat::DeDe)
        )
        .unwrap(),
        vec![vec![Value::Double(Double::from_float(100.0))]]
    );
}

#[test]
fn reused_financial_text_plans_keep_policy_and_eager_error_boundaries() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS R TEXT; SELECT Pmt(R,10,1000)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, text) in [(NumberFormat::DeDe, "0,0"), (NumberFormat::EnUs, "0.0")] {
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
            &[vec![Value::Double(Double::from_float(-100.0))]]
        );
    }
    for (expression, error) in [
        ("Pmt(NULL,'bad','1')", EvalError::InvalidNullConversion),
        ("Pmt('bad',NULL,1/0)", EvalError::DivisionByZero),
        ("EuroConvert(1,'USD','EUR','bad')", EvalError::Domain),
    ] {
        assert_eq!(run(expression, None), Err(error.into()), "{expression}");
    }
}
