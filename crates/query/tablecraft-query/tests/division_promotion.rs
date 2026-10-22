use tablecraft_core::values::{Double, Single, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn unary_and_quotient_carriers_survive_bound_execution() {
    assert_eq!(run("SELECT -CByte(1),-CInt(-32768),-CLng(-2147483648),-CSng(1.25),CSng(3)/CInt(2),CCur(3)/CCur(2),CCur(3)^2,CByte(9)\\CByte(2),CInt(9)\\CDbl(2.5),CDbl(9.5)\\CInt(2),'-9.5' Mod CInt(3)").unwrap(), [vec![
        Value::Int16(-1),Value::Int32(32768),Value::Double(Double::from_float(2147483648.0)),Value::Single(Single::from_float(-1.25)),Value::Single(Single::from_float(1.5)),Value::Double(Double::from_float(1.5)),Value::Double(Double::from_float(9.0)),Value::Byte(4),Value::Int16(4),Value::Int32(5),Value::Int32(-1),
    ]]);
    assert_eq!(run("SELECT VarType(-CByte(1)),VarType(-CInt(-32768)),VarType(-CLng(-2147483648)),VarType(-CSng(1)),VarType(CSng(3)/CInt(2)),VarType(CSng(3)/CLng(2)),VarType(CByte(9)\\CByte(2)),VarType(CInt(9)\\CDbl(2.5)),VarType(CDbl(9.5)\\CInt(2))").unwrap(), [vec![Value::Int16(2),Value::Int16(3),Value::Int16(5),Value::Int16(4),Value::Int16(4),Value::Int16(5),Value::Int16(17),Value::Int16(2),Value::Int16(3)]]);
}

#[test]
fn operand_bounds_eager_errors_and_derived_aggregate_consumers_remain_explicit() {
    for sql in [
        "SELECT -CLngLng(-9223372036854775808)",
        "SELECT CInt(9)\\'32767.5'",
        "SELECT '2147483647.5' Mod '2'",
        "SELECT CCur(1)/CCur(0)",
        "SELECT Null+(CCur(1)/CCur(0))",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
    assert_eq!(
        run("SELECT CInt(9)\\(1/0)").err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(run("SELECT Sum(X),VarType(First(X)) FROM (SELECT -CSng(1) AS X UNION ALL SELECT CSng(4)/CInt(2)) AS Q").unwrap(), [vec![Value::Double(Double::from_float(1.0)),Value::Int16(4)]]);
    assert_eq!(
        run("SELECT VarType((SELECT Q.X)) FROM (SELECT CByte(9)\\CByte(2) AS X) AS Q").unwrap(),
        [vec![Value::Int16(17)]]
    );
}
