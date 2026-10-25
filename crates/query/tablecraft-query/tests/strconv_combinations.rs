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
fn combined_transformations_preserve_derived_correlated_and_grouped_queries() {
    let derived = "SELECT StrConv(Q.T,37,1041),(SELECT StrConv(Q.T,24,1041)) FROM (SELECT 'aB ｶﾞ が' AS T UNION ALL SELECT NULL) Q";
    assert_eq!(
        run(derived).unwrap(),
        [
            vec![
                Value::Text("ＡＢ　が　が".into()),
                Value::Text("aB ｶﾞ ｶﾞ".into())
            ],
            vec![Value::Null, Value::Null],
        ]
    );
    let grouped = "SELECT StrConv(First(Q.T),43,1041),Count(*) FROM (SELECT 'aｶﾞ' AS T UNION ALL SELECT 'aｶﾞ') Q GROUP BY Len(StrConv(Q.T,43,1041))";
    assert_eq!(
        run(grouped).unwrap(),
        [vec![Value::Text("Aｶﾞ".into()), Value::Int64(2)]]
    );

    let raw = "SELECT StrConv(CVar(MidB('aaｶﾞ',5,4)),37,1041),LenB(StrConv('ｶﾞ',24,1041))";
    assert_eq!(
        run(raw).unwrap(),
        [vec![Value::Text("が".into()), Value::Int32(4)]]
    );
}

#[test]
fn prepared_combination_controls_and_failed_runs_never_capture_a_prior_locale() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "PARAMETERS P TEXT,M LONG,L LONG; SELECT StrConv(P,M,L)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (code, locale, expected) in [
        (5, 1041, Ok(Value::Text("ＡＢ　ガ　が".into()))),
        (37, 1033, Err(EvalError::UnsupportedExpression)),
        (37, 1041, Ok(Value::Text("ＡＢ　が　が".into()))),
        (100, 1041, Err(EvalError::UnsupportedExpression)),
        (24, 1041, Ok(Value::Text("aB ｶﾞ ｶﾞ".into()))),
        (40, 1041, Ok(Value::Text("aB ｶﾞ が".into()))),
    ] {
        let result = plan.execute(
            &source,
            &[
                ("P", Value::Text("aB ｶﾞ が".into())),
                ("M", Value::Int32(code)),
                ("L", Value::Int32(locale)),
            ],
            ExecutionOptions::default(),
        );
        match expected {
            Ok(value) => assert_eq!(result.unwrap().rows(), [vec![value]]),
            Err(error) => assert_eq!(result.err(), Some(QueryError::Evaluation(error))),
        }
    }
}

#[test]
fn combined_calls_keep_eager_failures_and_unknown_or_conflicting_flags_explicit() {
    for (sql, expected) in [
        (
            "SELECT StrConv(ChrW(55296),37,1041)",
            EvalError::UnrepresentableText,
        ),
        (
            "SELECT StrConv(ChrW(55296),37,1/0)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT StrConv(IIf(True,'x',1/0),5,1041)",
            EvalError::DivisionByZero,
        ),
        ("SELECT StrConv(NULL,44,1041)", EvalError::Domain),
        (
            "SELECT StrConv(NULL,100,1041)",
            EvalError::UnsupportedExpression,
        ),
        (
            "SELECT StrConv(NULL,37,66577)",
            EvalError::UnsupportedExpression,
        ),
    ] {
        assert_eq!(run(sql), Err(QueryError::Evaluation(expected)), "{sql}");
    }
}
