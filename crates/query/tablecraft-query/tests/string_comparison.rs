use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode},
};
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Words".into(),
            vec![
                Column::new("Unused".into(), ScalarType::Int32, true).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                Some("ABCD"),
                Some("abcd"),
                None,
                Some("😀"),
                Some("\u{e000}"),
            ]
            .into_iter()
            .map(|s| {
                vec![
                    Value::Null,
                    s.map_or(Value::Null, |s| Value::Text(s.into())),
                ]
            })
            .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, collation: Option<&dyn Collation>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}
struct Host;
impl Collation for Host {
    fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(a.cmp(b))
    }
    fn compare_text(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()))
    }
    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Text)
    }
}
#[test]
fn strcomp_binds_prunes_groups_and_correlates_without_implicit_sql_text_order() {
    assert_eq!(run("SELECT StrComp(Label,'abcd',0) AS C,Count(*) FROM Words GROUP BY StrComp(Label,'abcd',0) ORDER BY C",None).unwrap(),[vec![Value::Null,Value::Int64(1)],vec![Value::Int16(-1),Value::Int64(1)],vec![Value::Int16(0),Value::Int64(1)],vec![Value::Int16(1),Value::Int64(2)]]);
    assert_eq!(
        run(
            "SELECT (SELECT StrComp(W.Label,'abcd',0)) AS C FROM Words AS W ORDER BY C",
            None
        )
        .unwrap(),
        [
            vec![Value::Null],
            vec![Value::Int16(-1)],
            vec![Value::Int16(0)],
            vec![Value::Int16(1)],
            vec![Value::Int16(1)]
        ]
    );
    assert_eq!(run("SELECT StrComp('A','a',1),StrComp('A','a',2),StrComp('A','a'),StrComp('A','a',-1),StrComp('A','a',)",Some(&Host)).unwrap(),[vec![Value::Int16(0),Value::Int16(-1),Value::Int16(0),Value::Int16(0),Value::Int16(0)]]);
}
#[test]
fn strcomp_parameter_reuse_modes_and_eager_failures_keep_exact_carriers() {
    let source = source();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT StrComp(P,'a',0)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (input, result) in [
        (Value::Text("a".into()), Value::Int16(0)),
        (Value::Text("b".into()), Value::Int16(1)),
        (Value::Null, Value::Null),
    ] {
        assert_eq!(
            plan.execute(&source, &[("P", input)], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [vec![result]]
        );
    }
    assert_eq!(
        run("SELECT StrComp(NULL,'a',1/0)", None).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    for sql in [
        "SELECT StrComp('a','a')",
        "SELECT StrComp('a','a',2)",
        "SELECT StrComp(NULL,'a',NULL)",
        "SELECT StrComp('a',,0)",
        "SELECT IIf(False,StrComp('a','a',3),0)",
    ] {
        assert!(run(sql, None).is_err(), "{sql}");
    }
}
