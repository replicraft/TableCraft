use tablecraft_core::values::{Currency, Double, Single, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn typed_arithmetic_retains_widths_and_widens_at_materialized_boundaries() {
    assert_eq!(run("SELECT CByte(1)+CByte(2),CByte(255)+CByte(1),CInt(32767)+CInt(1),CLng(2147483647)+CInt(1),CLngLng(1)+CByte(1),CSng(1.25)+CInt(2),CSng(1.25)+CLng(2),CCur(1.5)*CCur(1.5)").unwrap(), [vec![
        Value::Byte(3), Value::Int16(256), Value::Int32(32768), Value::Double(Double::from_float(2147483648.0)), Value::Int64(2), Value::Single(Single::from_float(3.25)), Value::Double(Double::from_float(3.25)), Value::Currency(Currency::from_scaled(22500)),
    ]]);
    assert_eq!(run("SELECT VarType(CByte(1)+CByte(2)),VarType(CByte(255)+CByte(1)),VarType(CInt(32767)+CInt(1)),VarType(CLng(2147483647)+CInt(1)),VarType(CLngLng(1)+CByte(1)),VarType(CSng(1.25)+CInt(2)),VarType(CSng(1.25)+CLng(2)),VarType(CCur(1.5)*CCur(1.5)),VarType(CDate(2.25)+CDec(1))").unwrap(), [vec![Value::Int16(17),Value::Int16(2),Value::Int16(3),Value::Int16(5),Value::Int16(20),Value::Int16(4),Value::Int16(5),Value::Int16(6),Value::Int16(14)]]);
    assert_eq!(
        run("SELECT CLngLng(9223372036854775807)+CByte(1)").err(),
        Some(QueryError::Evaluation(EvalError::Overflow))
    );
}

#[test]
fn promoted_rows_remain_valid_through_grouping_correlations_and_pivots() {
    assert_eq!(run("SELECT TypeName(Q.X),Sum(Q.X) FROM (SELECT CByte(1)+CByte(2) AS X UNION ALL SELECT CByte(2)+CByte(1)) AS Q GROUP BY Q.X").unwrap(), [vec![Value::Text("Byte".into()), Value::Int64(6)]]);
    assert_eq!(
        run("SELECT VarType((SELECT Q.X)) FROM (SELECT CByte(1)+CByte(2) AS X) AS Q").unwrap(),
        [vec![Value::Int16(17)]]
    );
    assert_eq!(run("TRANSFORM First(CSng(X)+CInt(1)) SELECT CByte(1)+CByte(1) AS G FROM (SELECT 1 AS X UNION ALL SELECT 2) AS Q GROUP BY CByte(1)+CByte(1) PIVOT X IN (1,2)").unwrap(), [vec![Value::Byte(2),Value::Single(Single::from_float(2.0)),Value::Single(Single::from_float(3.0))]]);
    assert_eq!(run("SELECT CByte(1)+CByte(2) FROM (SELECT 1 AS X UNION ALL SELECT 2) AS Q WHERE IIf(X=1,True,CLngLng(9223372036854775807)+CByte(1)>0)").err(), Some(QueryError::Evaluation(EvalError::Overflow)));
}

#[test]
fn double_selected_arithmetic_rounds_large_parameters_and_retains_integer_paths() {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query(
        concat!(
            "PARAMETERS P BIGINT; SELECT P+CDbl(0),P/CLngLng(1),",
            "P^CByte(1),VarType(P+CSng(0)),P+CByte(0)"
        ),
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (integer, rounded) in [
        (9007199254740993, 9007199254740992.0),
        (9007199254740995, 9007199254740996.0),
        (-9007199254740993, -9007199254740992.0),
        (i64::MIN, -9223372036854775808.0),
        (i64::MAX, 9223372036854775808.0),
    ] {
        let double = Value::Double(Double::from_float(rounded));
        let result = plan
            .execute(&source, &[("P", Value::Int64(integer))], Default::default())
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                double.clone(),
                double.clone(),
                double,
                Value::Int16(5),
                Value::Int64(integer)
            ]]
        );
    }
    let aggregate = run(concat!(
        "SELECT (SELECT 9007199254740993+0.0),Sum(Q.N) FROM ",
        "(SELECT 9007199254740993+0.0 AS N UNION ALL ",
        "SELECT -9007199254740993+0.0) AS Q"
    ))
    .unwrap();
    assert_eq!(
        aggregate,
        [vec![
            Value::Double(Double::from_float(9007199254740992.0)),
            Value::Double(Double::from_float(0.0))
        ]]
    );
    assert!(matches!(
        run("SELECT 9007199254740993/0"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
    assert!(matches!(
        run("SELECT Null+(9007199254740993/0)"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
    // Strict predicate comparison remains separate from arithmetic conversion.
    assert!(matches!(
        run("SELECT 9007199254740993=0.0"),
        Err(QueryError::Evaluation(EvalError::PrecisionLoss))
    ));
}
