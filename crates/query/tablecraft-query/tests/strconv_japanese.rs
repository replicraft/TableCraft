//! Explicit Japanese string-codec conformance; no Windows/NLS equivalence claim.
#[path = "strconv_japanese/boundaries.rs"]
mod boundaries;

use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::text::code_page::CharacterCodePage};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source)?;
    Ok(plan
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn known_japanese_bytes_round_trip_without_using_a_python_codec() {
    let sql = "SELECT StrConv(StrConv('日本',128,1041),64,1041),LenB(StrConv('日本',128,1041)),AscB(MidB(StrConv('日本',128,1041),1,1)),AscB(MidB(StrConv('日本',128,1041),2,1)),AscB(MidB(StrConv('日本',128,1041),3,1)),AscB(MidB(StrConv('日本',128,1041),4,1))";
    let expected = vec![
        Value::Text("日本".into()),
        Value::Int32(4),
        Value::Int16(0x93),
        Value::Int16(0xfa),
        Value::Int16(0x96),
        Value::Int16(0x7b),
    ];
    assert_eq!(run(sql).unwrap(), [expected]);

    let halfwidth = "SELECT StrConv(StrConv('ｶﾞ',128,1041),64,1041),AscB(StrConv('ｶﾞ',128,1041)),AscB(MidB(StrConv('ｶﾞ',128,1041),2,1))";
    assert_eq!(
        run(halfwidth).unwrap(),
        [vec![
            Value::Text("ｶﾞ".into()),
            Value::Int16(0xb6),
            Value::Int16(0xde)
        ]]
    );

    // Both legacy duplicates decode identically; the WHATWG encoder chooses FA40.
    let duplicate = "SELECT StrConv(ChrW(61422),64,1041),StrConv(ChrW(16634),64,1041),AscB(StrConv('ⅰ',128,1041)),AscB(MidB(StrConv('ⅰ',128,1041),2,1))";
    assert_eq!(
        run(duplicate).unwrap(),
        [vec![
            Value::Text("ⅰ".into()),
            Value::Text("ⅰ".into()),
            Value::Int16(0xfa),
            Value::Int16(0x40),
        ]]
    );
}

#[test]
fn grouped_correlated_and_aggregate_consumers_keep_multibyte_inputs() {
    let grouped = "SELECT LenB(StrConv(Q.T,128,1041)) AS Width,Count(*) FROM (SELECT '日本' AS T UNION ALL SELECT '東京' UNION ALL SELECT 'ｶﾞ') Q GROUP BY LenB(StrConv(Q.T,128,1041)) ORDER BY Width";
    assert_eq!(
        run(grouped).unwrap(),
        [
            vec![Value::Int32(2), Value::Int64(1)],
            vec![Value::Int32(4), Value::Int64(2)],
        ]
    );
    let correlated = "SELECT (SELECT StrConv(StrConv(Q.T,128,1041),64,1041)) FROM (SELECT '日本' AS T UNION ALL SELECT NULL) Q";
    assert_eq!(
        run(correlated).unwrap(),
        [vec![Value::Text("日本".into())], vec![Value::Null]]
    );
    let aggregate = "SELECT StrConv(StrConv(First(Q.T),128,1041),64,1041) FROM (SELECT '日本' AS T UNION ALL SELECT '東京') Q";
    assert_eq!(run(aggregate).unwrap(), [vec![Value::Text("日本".into())]]);
}

#[test]
fn byte_decoding_uses_only_active_ranges_and_accepts_explicit_binary() {
    let raw =
        "SELECT StrConv(CVar(MidB(ChrW(55296)&ChrW(64147)&ChrW(31638)&ChrW(56320),3,4)),64,1041)";
    assert_eq!(run(raw).unwrap(), [vec![Value::Text("日本".into())]]);
    assert!(run("SELECT StrConv(MidB(ChrW(17025),1,1),64,1041)").is_err());

    let source = MemorySource::default();
    let syntax = parse_query(
        "PARAMETERS P VALUE; SELECT StrConv(P,64,1041)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let output = plan
        .execute(
            &source,
            &[("P", Value::Binary(vec![0x93, 0xfa, 0x96, 0x7b, 0]))],
            Default::default(),
        )
        .unwrap();
    assert_eq!(output.rows(), [vec![Value::Text("日本\0".into())]]);

    assert_eq!(
        run("SELECT LenB(StrConv('A日本',128,1041))").unwrap(),
        [vec![Value::Int32(5)]]
    );
    assert!(run("SELECT StrConv('A日本',128,1041)").is_err());
}

#[test]
fn a_reused_plan_keeps_the_call_locale_separate_from_single_byte_global_policy() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "PARAMETERS P TEXT,L LONG; SELECT LenB(StrConv(P,128,L)),StrConv(StrConv(P,128,L),64,L)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let page = CharacterCodePage::from_windows_id(1252).unwrap();
    for (text, locale, bytes) in [("日本", 1041, 4), ("é", 1033, 1), ("日本", 1041, 4)] {
        let output = plan
            .execute(
                &source,
                &[("P", Value::Text(text.into())), ("L", Value::Int32(locale))],
                ExecutionOptions {
                    character_code_page: Some(page),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            output.rows(),
            [vec![Value::Int32(bytes), Value::Text(text.into())]]
        );
    }
    assert!(CharacterCodePage::from_windows_id(932).is_err());
    assert!(run("SELECT StrConv('日本',128)").is_err());
    assert!(run("SELECT StrConv('日本',128,66577)").is_err());
}
