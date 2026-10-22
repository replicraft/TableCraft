use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn concatenation_rejects_error_data_after_evaluation_but_allows_explicit_cstr() {
    for sql in [
        "SELECT CVErr(0)&''",
        "SELECT ''&CVErr(65535)",
        "SELECT CVErr(0)&Null",
        "SELECT Null&CVErr(0)",
        "SELECT CVErr(0)&CVErr(1)",
        "SELECT Problem&'!' FROM (SELECT CVErr(1) AS Problem) AS Q",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::TypeMismatch.into()),
            "{sql}"
        );
    }
    for sql in ["SELECT CVErr(0)&(1/0)", "SELECT (1/0)&CVErr(0)"] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT CStr(CVErr(2001))&'!' FROM (SELECT 1 AS ID) AS Q").unwrap(),
        [vec![Value::Text("Error 2001!".into())]]
    );
}

#[test]
fn numeric_and_statistical_aggregates_require_explicit_error_code_conversion() {
    for function in ["Sum", "Avg", "Var", "VarP", "StDev", "StDevP"] {
        for expression in ["CVErr(ID)", "IIf(ID=1,1,CVErr(ID))"] {
            let sql = format!(
                "SELECT {function}({expression}) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q"
            );
            assert_eq!(
                run(&sql).err(),
                Some(EvalError::TypeMismatch.into()),
                "{sql}"
            );
        }
        let sql = format!("SELECT {function}(IIf(True,CVErr(0),1/0))");
        assert_eq!(
            run(&sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT Sum(CLng(CVErr(ID))) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q").unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert_eq!(
        run("SELECT Sum(CVErr(-1)),Avg(CVErr(-1)),Var(CVErr(-1)) FROM (SELECT 1 AS ID) AS Q WHERE ID=0").unwrap(),
        [vec![Value::Null; 3]]
    );
}
