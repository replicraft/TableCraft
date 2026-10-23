use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn recognition_and_state_predicates_keep_scalar_boolean_returns() {
    for expression in [
        "IsNumeric(CVar(1))",
        "IsDate(CVar(CDate(1)))",
        "IsError(CVErr(5))",
        "IsArray(Array(1))",
    ] {
        for sql in [
            format!("SELECT CInt(-32768)\\({expression})"),
            format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT (SELECT CInt(-32768)\\Q.N) FROM (SELECT {expression} AS N) AS Q"),
        ] {
            assert_eq!(run(&sql), Err(EvalError::Overflow.into()), "{sql}");
        }
        let sql = format!("SELECT CInt(-32768)\\CVar({expression})");
        assert_eq!(run(&sql).unwrap(), vec![vec![Value::Int32(32768)]], "{sql}");
    }
    assert_eq!(
        run("SELECT CInt(-32768)\\IsEmpty(CVar(1))"),
        Err(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\IsNumeric(1/0)"),
        Err(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\IsError(1/0)"),
        Err(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT CInt(-32768)\\TypeName(CInt(1))"),
        Err(EvalError::NumberFormatRequired.into())
    );
}
