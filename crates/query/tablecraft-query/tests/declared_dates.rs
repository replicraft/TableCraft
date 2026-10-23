use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(expression: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
    let source = MemorySource::default();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn declared_date_binary_and_unary_overflow_preserve_variant_widening() {
    for (scalar, variant, expected) in [
        ("CDate(2958465)+1", "CVDate(2958465)+1", 2958466.0),
        ("1+CDate(2958465)", "CVar(1)+CDate(2958465)", 2958466.0),
        ("CDate(-657434)-1", "CDate(-657434)-CVar(1)", -657435.0),
        ("-CDate(2958465)", "-CVDate(2958465)", -2958465.0),
        (
            "(SELECT CDate(2958465))+1",
            "(SELECT CVDate(2958465))+1",
            2958466.0,
        ),
        (
            "CDate(2958465)+CDate(1)",
            "CDate(2958465)+CVDate(1)",
            2958466.0,
        ),
    ] {
        assert_eq!(
            run(scalar, true),
            Err(EvalError::Overflow.into()),
            "{scalar}"
        );
        let result = vec![vec![Value::Double(Double::from_float(expected))]];
        assert_eq!(run(variant, true).unwrap(), result, "{variant}");
        assert_eq!(run(scalar, false).unwrap(), result, "default {scalar}");
    }
    assert_eq!(
        run("CDate(2958465)-CDate(-657434)", true).unwrap(),
        vec![vec![Value::Double(Double::from_float(3615899.0))]]
    );
}

#[test]
fn contextual_numeric_text_keeps_date_overflow_categories_and_explicit_policies() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("CDate(2958465)+'1,0'", Err(EvalError::Overflow.into())),
        ("'1,0'+CDate(2958465)", Err(EvalError::Overflow.into())),
        ("CVDate(2958465)+'1,0'", Ok(2958466.0)),
        ("CDate(2958465)+CVar('1,0')", Ok(2958466.0)),
        ("CDate(-657434)-'1,0'", Err(EvalError::Overflow.into())),
        ("CDate(-657434)-CVar('1,0')", Ok(-657435.0)),
        ("'0,0'-CVDate(2958465)", Ok(-2958465.0)),
    ] {
        let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            },
        );
        match expected {
            Ok(serial) => assert_eq!(
                result.unwrap().rows(),
                &[vec![Value::Double(Double::from_float(serial))]],
                "{expression}"
            ),
            Err(error) => assert_eq!(result.err(), Some(error), "{expression}"),
        }
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    ..Default::default()
                }
            )
            .err(),
            Some(EvalError::NumberFormatRequired.into()),
            "{expression}"
        );
    }
}
