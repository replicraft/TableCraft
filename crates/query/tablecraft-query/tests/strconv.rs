use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::text::code_page::CharacterCodePage};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn strconv_expression_states_keep_controls_before_source_and_raw_intermediates() {
    for (sql, expected) in [
        (
            "SELECT StrConv(E,-1) FROM (SELECT CVErr(11) AS E) AS D",
            EvalError::Domain,
        ),
        ("SELECT StrConv(CVar(CVErr(11)),0,0)", EvalError::Domain),
        ("SELECT StrConv(NULL,CVErr(11))", EvalError::TypeMismatch),
        ("SELECT StrConv(CVErr(11),0,1/0)", EvalError::DivisionByZero),
        ("SELECT StrConv(ChrB(300),CVErr(11))", EvalError::Overflow),
    ] {
        assert_eq!(run(sql), Err(QueryError::Evaluation(expected)), "{sql}");
    }
    assert_eq!(
        run("SELECT LenB(StrConv(CVar(MidB('😀',1,1)),0)),StrConv(CStr(CVErr(11)),2,1033),StrConv('abc',CLng(CVErr(1)),1033)").unwrap(),
        [vec![Value::Int32(1), Value::Text("error 11".into()), Value::Text("ABC".into())]]
    );
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn strconv_retains_group_identity_correlations_aggregates_and_binary_inputs() {
    let grouped = run("SELECT LenB(StrConv(A.T,128,1033)),Count(*) FROM (SELECT 'é€' AS T UNION ALL SELECT 'ab') AS A GROUP BY LenB(StrConv(A.T,128,1033))").unwrap();
    assert_eq!(grouped, [vec![Value::Int32(2), Value::Int64(2)]]);
    let correlated =
        run("SELECT (SELECT StrConv(A.T,1,1033)) FROM (SELECT 'Hello' AS T) AS A").unwrap();
    assert_eq!(correlated, [vec![Value::Text("HELLO".into())]]);
    let aggregate =
        run("SELECT StrConv(First(A.T),3,1033) FROM (SELECT 'hELLO' AS T) AS A").unwrap();
    assert_eq!(aggregate, [vec![Value::Text("Hello".into())]]);
    let binary = run("SELECT StrConv(A.B,64,1033) FROM (SELECT GUIDFromString('00000000-0000-0000-0000-000000000000') AS B) AS A").unwrap();
    assert_eq!(binary, [vec![Value::Text("\0".repeat(16))]]);
    assert_eq!(run("SELECT LenB(StrConv(A.T,128,1033)) FROM (SELECT 'x' AS T) AS A GROUP BY LenB(StrConv(A.T,128,1036))").err(), Some(QueryError::Grouping));
}

#[test]
fn strconv_parameter_modes_and_call_local_policies_do_not_leak_between_executions() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT,M LONG,L LONG; SELECT StrConv(P,M,L)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (mode, locale, expected) in [(1, 1033, "I"), (1, 1055, "İ"), (2, 1033, "i")] {
        assert_eq!(
            plan.execute(
                &source,
                &[
                    ("P", Value::Text("i".into())),
                    ("M", Value::Int32(mode)),
                    ("L", Value::Int32(locale))
                ],
                Default::default()
            )
            .unwrap()
            .rows(),
            [vec![Value::Text(expected.into())]]
        );
    }
    let parsed = parse_query(
        "SELECT AscW(StrConv(ChrB(192),64)),AscB(StrConv(StrConv(ChrB(192),64),128))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (page, code) in [
        (Some(1252), Some(192)),
        (Some(1251), Some(1040)),
        (None, None),
        (Some(1252), Some(192)),
    ] {
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                character_code_page: page.map(|id| CharacterCodePage::from_windows_id(id).unwrap()),
                ..Default::default()
            },
        );
        if let Some(code) = code {
            assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Int16(code), Value::Int16(192)]]
            );
        } else {
            assert_eq!(
                result.err(),
                Some(EvalError::CharacterPolicyRequired.into())
            );
        }
    }
    let language = "tr".parse().unwrap();
    let parsed = parse_query("SELECT StrConv('i',1,1033),StrConv('i',1),AscB(StrConv('€',128,1033)),AscB(StrConv('€',128))", Default::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                case_locale: Some(&language),
                character_code_page: Some(CharacterCodePage::from_windows_id(1251).unwrap()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            Value::Text("I".into()),
            Value::Text("İ".into()),
            Value::Int16(128),
            Value::Int16(136)
        ]]
    );
}

#[test]
fn strconv_query_cancellation_and_refusal_keep_the_existing_failure_boundary() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT StrConv(StrConv('hELLO WORLD',131,1033),64,1033)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        let cancel = || {
            checkpoints.set(checkpoints.get() + 1);
            checkpoints.get() == stop
        };
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    for sql in [
        "SELECT StrConv('x',1,1033,1/0)",
        "SELECT StrConv(*)",
        "SELECT StrConv(,1)",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported));
    }
    assert_eq!(
        run("SELECT StrConv(Null,1,1/0)").err(),
        Some(EvalError::DivisionByZero.into())
    );
}
