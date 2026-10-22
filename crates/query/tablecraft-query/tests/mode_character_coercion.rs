use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn characters_bytes_and_string_modes_convert_only_numeric_slots() {
    assert_eq!(run("SELECT Asc(Chr('65,5')),AscW(ChrW('65,5')),AscB(ChrB('65,5')),LenB(LeftB('AB','2,5')),AscB(MidB('AB','2','1')),InStrB('1','AB',ChrB('66'),'0')", Some(NumberFormat::DeDe)).unwrap(), [vec![Value::Int16(66),Value::Int16(66),Value::Int16(66),Value::Int32(2),Value::Int16(0),Value::Int32(3)]]);
    assert_eq!(run("SELECT StrComp('10','2','0,5'),InStr(1,'abc','b','0,5'),InStrRev('abc','b',-1,'0,5'),Replace('banana','an','X','2,5','2,5','0,5'),StrComp(NULL,'x','-1')", Some(NumberFormat::DeDe)).unwrap(), [vec![Value::Int16(-1),Value::Int32(2),Value::Int32(2),Value::Text("XXa".into()),Value::Null]]);
    assert_eq!(run("SELECT Asc(Chr(CDate(65.5))),AscB(ChrB(CDate(65.5))),StrComp('10','2',CDate(0.5)),LenB(LeftB('AB',CDate(2.5)))", None).unwrap(), [vec![Value::Int16(66),Value::Int16(66),Value::Int16(-1),Value::Int32(2)]]);
}

#[test]
fn prepared_modes_rebind_number_policies_in_nested_queries() {
    let source = MemorySource::default();
    let query = parse_query("PARAMETERS Code TEXT,Mode TEXT; SELECT AscW(ChrW(Code)),(SELECT StrComp('10','2',Mode)),Eval(\"Asc(Chr('65'))\")", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for text in ["65.5", "66.5"] {
        let bindings = [
            ("Code", Value::Text(text.into())),
            ("Mode", Value::Text("0.5".into())),
        ];
        let result = plan
            .execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int16(66), Value::Int16(-1), Value::Int16(65)]]
        );
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
    }
}

#[test]
fn eager_errors_pages_surrogates_and_unsupported_modes_remain_explicit() {
    for sql in [
        "SELECT InStr('bad','abc','b',1/0)",
        "SELECT StrComp(NULL,1/0,'bad')",
        "SELECT Replace(NULL,'a','b','bad',1/0,'bad')",
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    for sql in [
        "SELECT Chr('65')",
        "SELECT StrComp('a','b','0')",
        "SELECT AscB(ChrB('65'))",
    ] {
        assert_eq!(
            run(sql, None),
            Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
    for sql in [
        "SELECT Chr('128')",
        "SELECT ChrW('55296')",
        "SELECT InStrB(1,'abc','b','2')",
        "SELECT InStrRev('abc','b',-1,'1033')",
        "SELECT Asc(65)",
        "SELECT StrConv('abc',0,'1033')",
    ] {
        assert!(run(sql, Some(NumberFormat::EnUs)).is_err(), "{sql}");
    }
}
