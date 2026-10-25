use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn japanese_word_boundaries_compose_through_width_encoding_and_borrowed_rows() {
    let sql = "SELECT StrConv(Q.T,3,1041),(SELECT StrConv(Q.T,11,1041)) FROM (SELECT 'hello　WORLD' AS T UNION ALL SELECT NULL) Q";
    assert_eq!(
        run(sql).unwrap(),
        [
            vec![
                Value::Text("Hello　World".into()),
                Value::Text("Hello World".into())
            ],
            vec![Value::Null, Value::Null],
        ]
    );
    let encoded = "SELECT StrConv(StrConv('hello　WORLD',131,1041),64,1041)";
    assert_eq!(
        run(encoded).unwrap(),
        [vec![Value::Text("Hello　World".into())]]
    );

    let raw = "SELECT StrConv(MidB(ChrW(55296)&'hello　WORLD'&ChrW(56320),3,LenB('hello　WORLD')),3,1041)";
    assert_eq!(
        run(raw).unwrap(),
        [vec![Value::Text("Hello　World".into())]]
    );
    assert_eq!(
        run("SELECT StrConv(ChrW(55296),3,1041)"),
        Err(QueryError::Evaluation(EvalError::UnrepresentableText))
    );
}

#[test]
fn repeated_parameterized_calls_choose_each_explicit_locale_without_leaking_japanese() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "PARAMETERS L LONG; SELECT StrConv('hello　WORLD',3,L)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (locale, expected) in [
        (1041, Ok("Hello　World")),
        (1033, Ok("Hello　world")),
        (66577, Err(EvalError::UnsupportedExpression)),
        (1041, Ok("Hello　World")),
    ] {
        let actual = plan.execute(
            &source,
            &[("L", Value::Int32(locale))],
            ExecutionOptions::default(),
        );
        match expected {
            Ok(text) => assert_eq!(actual.unwrap().rows(), [vec![Value::Text(text.into())]]),
            Err(error) => assert_eq!(actual.err(), Some(QueryError::Evaluation(error))),
        }
    }
}
