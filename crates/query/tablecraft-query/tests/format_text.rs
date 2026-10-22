use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn text_layouts_preserve_group_aggregate_and_correlated_dependencies() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Len(Format(A.T,'@@@@')),Count(*) FROM (SELECT 'Ab' AS T UNION ALL SELECT 'Ab') AS A GROUP BY Len(Format(A.T,'@@@@'))",
            vec![Value::Int32(4), Value::Int64(2)],
        ),
        (
            "SELECT (SELECT Format(A.T,'!@@@@')) FROM (SELECT 'Ab' AS T) AS A",
            vec![Value::Text("Ab  ".into())],
        ),
        (
            "SELECT Format(First(A.T),'@@@@') FROM (SELECT 'Ab' AS T UNION ALL SELECT 'Cd') AS A",
            vec![Value::Text("  Ab".into())],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [expected]);
    }
}

#[test]
fn reused_text_layout_plans_resolve_masks_and_case_language_each_execution() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT,M TEXT; SELECT Format(P,M)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (language, mask, expected) in [
        (Some("tr"), ">@@", Some("İI")),
        (Some("und"), ">@@", Some("II")),
        (None, ">@@", None),
        (None, "@@@@", Some("  iı")),
    ] {
        let locale = language.map(|name| name.parse().unwrap());
        let result = plan.execute(
            &source,
            &[
                ("P", Value::Text("iı".into())),
                ("M", Value::Text(mask.into())),
            ],
            ExecutionOptions {
                case_locale: locale.as_ref(),
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Text(expected.into())]]);
        } else {
            assert_eq!(result.err(), Some(EvalError::CaseLocaleRequired.into()));
        }
    }
}
