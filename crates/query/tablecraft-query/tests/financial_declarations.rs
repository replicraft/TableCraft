use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
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
fn all_financial_returns_are_scalar_independently_of_arguments() {
    for expression in [
        "Pmt(CVar(0),1,-1)",
        "PV(CVar(0),1,-1)",
        "FV(CVar(0),1,-1)",
        "NPer(CVar(0),-1,1)",
        "IPmt(CVar(1),1,1,-1)",
        "PPmt(CVar(0),1,1,-1)",
        "SLN(CVar(1),0,1)",
        "SYD(CVar(1),0,1,1)",
        "DDB(CVar(1),0,1,1)",
        "Rate(CVar(1),-2,1,0,0,1)",
        "EuroConvert(CVar(1),'EUR','EUR')",
        "NPV(CVar(0),Array(1))",
        "IRR(Array(-1,2),CVar(1))",
        "MIRR(Array(-1,2),CVar(0),0)",
    ] {
        for sql in [
            format!("SELECT CInt(-32768)\\(-({expression}))"),
            format!("SELECT CInt(-32768)\\(-Q.N) FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT (SELECT CInt(-32768)\\(-Q.N)) FROM (SELECT {expression} AS N) AS Q"),
            format!(
                "SELECT CInt(-32768)\\(-Q.N) FROM (SELECT {expression} AS N) AS Q GROUP BY Q.N"
            ),
        ] {
            assert_eq!(run(&sql, true), Err(EvalError::Overflow.into()), "{sql}");
            assert_eq!(
                run(&sql, false).unwrap(),
                vec![vec![Value::Int32(32768)]],
                "{sql}"
            );
        }
        let sql = format!("SELECT CInt(-32768)\\CVar(-({expression}))");
        assert_eq!(
            run(&sql, true).unwrap(),
            vec![vec![Value::Int32(32768)]],
            "{sql}"
        );
    }
}

#[test]
fn declaration_checks_preserve_financial_argument_failures() {
    let sql = "SELECT CInt(-32768)\\(-Pmt(0,1,-1,1/0))";
    assert_eq!(run(sql, true), Err(EvalError::DivisionByZero.into()));
}
