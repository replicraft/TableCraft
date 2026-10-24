use tablecraft_core::values::{LegacyDateTime, Single, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(expression: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn unknown_binary_results_use_agreement_across_integral_single_date_and_rounding() {
    for (expression, expected) in [
        ("First(CByte(7))+CByte(2)", Value::Byte(9)),
        ("First(CInt(2))+First(CInt(3))", Value::Int16(5)),
        ("First(CInt(2))-CInt(3)", Value::Int16(-1)),
        ("First(CLng(32768))*CInt(2)", Value::Int32(65536)),
        ("First(CInt(7))\\CInt(2)", Value::Int16(3)),
        (
            "First(CSng(1.25))*CInt(2)",
            Value::Single(Single::from_float(2.5)),
        ),
        (
            "First(CDate(2.25))+CInt(1)",
            Value::DateTime(LegacyDateTime::from_ole_days(3.25).unwrap()),
        ),
        ("First('7,5')\\CInt(2)", Value::Int32(4)),
        ("First(CInt(7))\\'2,5'", Value::Int16(3)),
        (
            "First(CDate(2.25))+'0,5'",
            Value::DateTime(LegacyDateTime::from_ole_days(2.75).unwrap()),
        ),
    ] {
        assert_eq!(
            run(expression, Some(NumberFormat::DeDe)).unwrap(),
            [vec![expected]],
            "{expression}"
        );
    }
}

#[test]
fn widening_requires_provenance_and_agreement_does_not_assign_it() {
    for expression in [
        "First(CByte(255))+CByte(1)",
        "First(CInt(32767))+CInt(1)",
        "First(CLng(-2147483648))-CInt(1)",
        "First(CSng(3.4028234663852886e38))*CInt(2)",
        "First(CDate(2958465))+CInt(1)",
        "First(CInt(-32768))\\CInt(-1)",
        "CInt(-32768)\\((First(CInt(2))+CInt(1))=CInt(3))",
        "Q.N+CInt(32767) FROM (SELECT First(CInt(1))+CInt(1) AS N) AS Q",
    ] {
        assert_eq!(
            run(expression, None),
            Err(QueryError::ArithmeticDeclaration),
            "{expression}"
        );
    }
}

#[test]
fn common_errors_remain_operational_instead_of_becoming_declaration_failures() {
    for (expression, expected) in [
        ("First(CInt(2))\\CInt(0)", EvalError::DivisionByZero),
        ("First('bad')\\CInt(2)", EvalError::TypeMismatch),
        ("First(CInt(1))\\'32768'", EvalError::Overflow),
        ("First(CInt(2))+(1/0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(
            run(expression, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(expected)),
            "{expression}"
        );
    }
    assert_eq!(
        run("First('2')\\CInt(1)", None),
        Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}

#[test]
fn operand_evaluation_and_prepared_rebinding_do_not_manufacture_provenance() {
    let source = MemorySource::default();
    let random = RndState::new(7).unwrap();
    let reference = RndState::new(7).unwrap();
    reference.apply(None).unwrap();
    reference.apply(None).unwrap();
    let query = parse_query(
        "SELECT First(CInt(Rnd()*100))+CInt(Rnd()*100)",
        Default::default(),
    )
    .unwrap();
    prepare(query.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&random),
                declared_integral: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(random.apply(None).unwrap(), reference.apply(None).unwrap());

    let query = parse_query(
        "PARAMETERS P SHORT; SELECT Q.N+CInt(1) FROM (SELECT P AS N UNION ALL SELECT P WHERE False) AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for value in [2, i16::MAX, -2, i16::MIN] {
        let result = plan.execute(
            &source,
            &[("P", Value::Int16(value))],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        );
        if value == i16::MAX {
            assert_eq!(result.err(), Some(QueryError::ArithmeticDeclaration));
        } else {
            assert_eq!(result.unwrap().rows(), [vec![Value::Int16(value + 1)]]);
        }
    }
}
