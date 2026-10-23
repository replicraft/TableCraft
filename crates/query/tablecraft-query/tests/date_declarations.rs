use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn date_conversion_return_declarations_control_rounded_overflow() {
    let source = MemorySource::default();
    for (date, scalar) in [
        ("CDate(-1)", true),
        ("CVDate(-1)", false),
        ("CDate(CVar(-1))", true),
        ("CDate(CVDate(-1))", true),
        ("CVDate(CDate(-1))", false),
        ("CVar(CDate(-1))", false),
    ] {
        for sql in [
            format!("SELECT CInt(-32768)\\{date}"),
            format!("SELECT CInt(-32768)\\Q.D FROM (SELECT {date} AS D) AS Q"),
            format!("SELECT (SELECT CInt(-32768)\\Q.D) FROM (SELECT {date} AS D) AS Q"),
            format!("SELECT CInt(-32768)\\{date} FROM (SELECT 1 AS N) AS Q GROUP BY {date}"),
        ] {
            let parsed = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            for declared in [false, true] {
                let result = plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    },
                );
                if declared && scalar {
                    assert_eq!(
                        result.err(),
                        Some(QueryError::Evaluation(EvalError::Overflow)),
                        "{sql}"
                    );
                } else {
                    assert_eq!(
                        result.unwrap().rows(),
                        &[vec![Value::Int32(32768)]],
                        "{sql}"
                    );
                }
            }
        }
    }
}

#[test]
fn date_conversion_categories_keep_operand_errors_and_value_representations() {
    let source = MemorySource::default();
    for (sql, expected) in [
        ("SELECT CDate(NULL)", EvalError::InvalidNullConversion),
        ("SELECT CVDate(NULL)", EvalError::InvalidNullConversion),
        ("SELECT CInt(0)\\CDate(0)", EvalError::DivisionByZero),
        ("SELECT CInt(0)\\CVDate(0)", EvalError::DivisionByZero),
        ("SELECT CInt(-32768)\\CDate(1/0)", EvalError::DivisionByZero),
        (
            "SELECT CInt(-32768)\\CVDate(1/0)",
            EvalError::DivisionByZero,
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        for declared in [false, true] {
            assert_eq!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    }
                )
                .err(),
                Some(QueryError::Evaluation(expected)),
                "{sql}"
            );
        }
    }
    let parsed = parse_query(
        "SELECT VarType(CDate(-1)),VarType(CVDate(-1)),CDate(-1)=CVDate(-1)",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int16(7), Value::Int16(7), Value::Boolean(true)]]
    );
}
