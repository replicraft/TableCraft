use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::text::code_page::CharacterCodePage};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn prepared_nested_grouped_queries_use_execution_local_code_pages() {
    let source = MemorySource::default();
    let query=parse_query("PARAMETERS C LONG; SELECT String(2,C) AS Label,Count(*) FROM (SELECT 1 AS N) AS T GROUP BY String(2,C)",ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let nested = parse_query(
        "PARAMETERS C LONG; SELECT (SELECT String(2,C))",
        ParseLimits::default(),
    )
    .unwrap();
    let nested = prepare(nested.query(), &source).unwrap();
    for (page, expected) in [
        (Some(1252), Some("ÀÀ")),
        (Some(1251), Some("АА")),
        (None, None),
        (Some(1252), Some("ÀÀ")),
    ] {
        let result = plan.execute(
            &source,
            &[("C", Value::Int32(192))],
            ExecutionOptions {
                character_code_page: page.map(|id| CharacterCodePage::from_windows_id(id).unwrap()),
                ..Default::default()
            },
        );
        let nested_result = nested.execute(
            &source,
            &[("C", Value::Int32(192))],
            ExecutionOptions {
                character_code_page: page.map(|id| CharacterCodePage::from_windows_id(id).unwrap()),
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(
                nested_result.unwrap().rows(),
                [vec![Value::Text(expected.into())]]
            );
        } else {
            assert!(nested_result.is_err());
        }
        if let Some(expected) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Text(expected.into()), Value::Int64(1)]]
            );
        } else {
            assert_eq!(
                result.err(),
                Some(QueryError::Evaluation(EvalError::CharacterPolicyRequired))
            );
        }
    }
}
#[test]
fn code_pages_do_not_transcode_text_arguments_or_hide_unmapped_bytes() {
    let source = MemorySource::default();
    let query = parse_query(
        "SELECT String(2,'é'),String(2,448),String(NULL,170)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                character_code_page: Some(CharacterCodePage::from_windows_id(1251).unwrap()),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![
            Value::Text("éé".into()),
            Value::Text("АА".into()),
            Value::Null
        ]]
    );
    let query = parse_query("SELECT String(1,170)", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                character_code_page: Some(CharacterCodePage::from_windows_id(1253).unwrap()),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::InvalidCharacterCode))
    );
}
