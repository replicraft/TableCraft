use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn parameter_and_nested_lengths_use_each_execution_policy() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS N DOUBLE; SELECT Len(N),LenB(N),Len(CVar(N)),(SELECT LenB(N)),LenB(LeftB(N,3))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let parameters = [("N", Value::Double(Double::from_float(-12.5)))];
    for format in [NumberFormat::EnUs, NumberFormat::DeDe, NumberFormat::EnUs] {
        assert_eq!(
            plan.execute(
                &source,
                &parameters,
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![
                Value::Int32(5),
                Value::Int32(10),
                Value::Int32(5),
                Value::Int32(10),
                Value::Int32(3)
            ]]
        );
    }
    assert_eq!(
        plan.execute(&source, &parameters, Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}

#[test]
fn length_calls_keep_eager_errors_and_raw_byte_boundaries() {
    let source = MemorySource::default();
    for (sql, expected) in [
        ("SELECT Len(1/0)", EvalError::DivisionByZero),
        ("SELECT LenB(1/0)", EvalError::DivisionByZero),
        ("SELECT Len(LeftB('A',1))", EvalError::UnrepresentableText),
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(expected))
        );
    }
    let query = parse_query(
        "SELECT Len(NULL),LenB(NULL),LenB(LeftB('A',1)),Len('é中🦀'),LenB('é中🦀')",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![
            Value::Null,
            Value::Null,
            Value::Int32(1),
            Value::Int32(4),
            Value::Int32(8)
        ]]
    );
}
