use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
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
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn effective_numeric_predicates_compose_through_relational_consumers() {
    for predicate in [
        "CInt(2)='1.5'",
        "'2.5'=CByte(2)",
        "CLngLng(9007199254740993)='9007199254740993'",
        "CInt(2) IN ('1.5')",
        "CInt(2) IN (SELECT '1.5')",
        "CInt(2)=ALL(SELECT '1.5')",
        "CInt(2) BETWEEN '1.5' AND '2.5'",
        "CInt(2)=CVar('1.5')",
        "CVar(CInt(2))='1.5'",
        "CVar(CInt(2))<CVar('bad')",
        "CSng(1)=CDbl(1.0000000000000002)",
    ] {
        let sql = format!("SELECT {predicate}");
        assert_eq!(
            run(&sql, true).unwrap(),
            vec![vec![Value::Boolean(true)]],
            "{sql}"
        );
    }
    assert_eq!(run("SELECT A.N FROM (SELECT CInt(2) AS N) AS A INNER JOIN (SELECT '1.5' AS S) AS B ON A.N=B.S", true).unwrap(), vec![vec![Value::Int16(2)]]);
    assert_eq!(
        run("SELECT CInt(2)='1.5'", false).unwrap(),
        vec![vec![Value::Boolean(false)]]
    );
    assert_eq!(
        run("SELECT CVar(CInt(2))=CVar('2')", true).unwrap(),
        vec![vec![Value::Boolean(false)]]
    );
}

#[test]
fn selected_width_errors_null_and_eager_failures_remain_explicit() {
    for expression in ["CByte(1)='256'", "CInt(1)='32768'", "CLng(1)='2147483648'"] {
        assert_eq!(
            run(&format!("SELECT 1,{expression}"), true),
            Err(EvalError::Overflow.into())
        );
    }
    assert_eq!(
        run("SELECT NULL='bad'", true).unwrap(),
        vec![vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT CInt(2) IN ('1.5',1/0)", true),
        Err(EvalError::DivisionByZero.into())
    );
    assert!(
        run(
            "SELECT X,Count(*) FROM (SELECT CInt(2) AS X UNION ALL SELECT '1.5') AS Q GROUP BY X",
            true
        )
        .is_err()
    );
}

#[test]
fn date_numeric_selection_composes_without_enabling_implicit_extended_dates() {
    for predicate in [
        "CDate(1)=CInt(1)",
        "CInt(1)=CVDate(1)",
        "CDate(-1)=True",
        "CDate(-0.25)=CDbl(0.25)",
        "CDate(-1.25)<CDbl(-1.75)",
        "CDate(-0.25)=CDec(-0.25)",
        "CDate(-0.25)<>CDec(0.25)",
        "CDate(1)<CDec(2958466)",
        "CDate(1) IN (CInt(1))",
        "CDate(1) IN (SELECT CInt(1))",
        "CDate(1)=ALL(SELECT CInt(1))",
        "CDate(1) BETWEEN CInt(0) AND CInt(2)",
    ] {
        let sql = format!("SELECT {predicate}");
        assert_eq!(
            run(&sql, true).unwrap(),
            vec![vec![Value::Boolean(true)]],
            "{sql}"
        );
    }
    assert_eq!(run("SELECT A.N FROM (SELECT CInt(1) AS N) AS A INNER JOIN (SELECT CDate(1) AS D) AS B ON A.N=B.D", true).unwrap(), vec![vec![Value::Int16(1)]]);
    assert_eq!(
        run("SELECT CDate(1)=CInt(1)", false),
        Err(EvalError::TypeMismatch.into())
    );
    assert_eq!(
        run("SELECT 1,CDate(1)=CLng(2958466)", true),
        Err(EvalError::Overflow.into())
    );
    assert_eq!(
        run("SELECT CDate(1) IN (CInt(1),1/0)", true),
        Err(EvalError::DivisionByZero.into())
    );
}
