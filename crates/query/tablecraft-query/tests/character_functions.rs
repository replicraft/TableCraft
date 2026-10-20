use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::text::code_page::CharacterCodePage};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn character_calls_bind_group_nest_and_reuse_page_policies() {
    let source = MemorySource::default();
    let query=parse_query("PARAMETERS C LONG; SELECT Asc(Chr(C)) AS Code,Count(*) FROM (SELECT 1 AS N) AS T GROUP BY Asc(Chr(C))",ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for page in [1252, 1251, 866] {
        assert_eq!(
            plan.execute(
                &source,
                &[("C", Value::Int32(192))],
                ExecutionOptions {
                    character_code_page: Some(CharacterCodePage::from_windows_id(page).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Int16(192), Value::Int64(1)]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("C", Value::Int32(192))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::CharacterPolicyRequired))
    );
    let query = parse_query(
        "SELECT (SELECT AscW(ChrW(-1))),ChrW(8364),AscW('😀')",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![
            Value::Int16(-1),
            Value::Text("€".into()),
            Value::Int16(-10179)
        ]]
    );
}
#[test]
fn invalid_shapes_and_eager_errors_remain_visible() {
    let source = MemorySource::default();
    for sql in [
        "SELECT IIf(False,Asc(''),0)",
        "SELECT Chr(NULL)",
        "SELECT AscW(NULL)",
        "SELECT ChrW(55296)",
        "SELECT Chr(256)",
        "SELECT Asc(1)",
        "SELECT VBA.Chr(65)",
        "SELECT Chr(65,1/0)",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let result = prepare(query.query(), &source)
            .and_then(|plan| plan.execute(&source, &[], ExecutionOptions::default()));
        assert!(result.is_err(), "{sql}");
    }
}
