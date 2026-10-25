use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                declared_integral: true,
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn text_function_signatures_bind_without_executing_functions_or_providers() {
    for (variant, expressions) in [
        (
            true,
            &[
                "Len('a')",
                "LenB('a')",
                "Chr(65)",
                "ChrW(65)",
                "ChrB(65)",
                "LeftB('a',2)",
                "RightB('a',2)",
                "MidB('a',1,2)",
                "StrConv('a',1)",
                "LCase('a')",
                "UCase('a')",
                "LTrim('a')",
                "RTrim('a')",
                "Trim('a')",
                "Left('a',1)",
                "Right('a',1)",
                "Mid('a',1,1)",
                "StrComp('a','b',0)",
                "InStr('a','b')",
                "InStrB('a','b')",
                "Space(1)",
                "String(1,'a')",
                "Hex(1)",
                "Oct(1)",
                "Str(1)",
                "Format(1)",
                "Partition(1,0,10,1)",
            ][..],
        ),
        (
            false,
            &[
                "Asc('a')",
                "AscB('a')",
                "AscW('a')",
                "InStrRev('a','b')",
                "Val('1')",
                "CStr(1)",
                "Chr$(65)",
                "Replace('a','a','b')",
                "StrReverse('a')",
                "FormatDateTime(CDate(0))",
                "MonthName(1)",
                "WeekdayName(1)",
                "FormatNumber(1)",
                "FormatPercent(1)",
                "FormatCurrency(1)",
                "HyperlinkPart('a#b#',0)",
            ][..],
        ),
    ] {
        for expression in expressions {
            let sql =
                format!("SELECT CInt(-32768)\\(CInt(0)=ALL(SELECT {expression} WHERE False))");
            if variant {
                assert_eq!(run(&sql).unwrap(), vec![vec![Value::Int32(32768)]], "{sql}");
            } else {
                assert_eq!(run(&sql), Err(EvalError::Overflow.into()), "{sql}");
            }
        }
    }
}

#[test]
fn text_and_numeric_runtime_categories_survive_derived_and_scalar_subqueries() {
    for expression in [
        "Left('-1x',2)",
        "Right('x-1',2)",
        "Mid('x-1',2)",
        "Trim(' -1 ')",
        "Str(-1)",
    ] {
        for sql in [
            format!("SELECT CInt(-32768)\\({expression})"),
            format!("SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q"),
            format!("SELECT CInt(-32768)\\(SELECT {expression})"),
        ] {
            assert_eq!(run(&sql).unwrap(), vec![vec![Value::Int32(32768)]], "{sql}");
        }
    }
    for expression in [
        "Val('-1')",
        "CStr(-1)",
        "StrReverse('1-')",
        "Replace('x1','x','-')",
    ] {
        let sql = format!("SELECT CInt(-32768)\\({expression})");
        assert_eq!(run(&sql), Err(EvalError::Overflow.into()), "{sql}");
    }
    assert_eq!(
        run("SELECT CVar(65)<Chr(65),CVar(65)=Chr$(54)").unwrap(),
        vec![vec![Value::Boolean(true), Value::Boolean(false)]]
    );
    assert_eq!(
        run("SELECT CVar(65)<Chr$(65)").unwrap(),
        vec![vec![Value::Boolean(true)]]
    );
}
