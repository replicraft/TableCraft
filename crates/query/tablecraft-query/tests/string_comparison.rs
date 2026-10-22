use std::cmp::Ordering;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
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

#[test]
fn strcomp_formats_supported_scalars_without_numeric_comparison_or_stale_policies() {
    let source = source();
    let syntax = parse_query(
        concat!(
            "SELECT StrComp(12.5,'12,5',0),StrComp(True,'True',0),",
            "StrComp(CCur(1.25),'1,25',0),StrComp(CDec(12.345),'12,345',0),",
            "StrComp(#1900-01-01#,'1900-01-01',0),StrComp(10,2,0)"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(syntax.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                date_format: Some(DateTextFormat::Iso),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int16(0),
            Value::Int16(0),
            Value::Int16(0),
            Value::Int16(0),
            Value::Int16(0),
            Value::Int16(-1)
        ]]
    );
    let syntax = parse_query(
        "PARAMETERS P DOUBLE; SELECT StrComp(P,'12,5',0)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (format, expected) in [(NumberFormat::EnUs, 1), (NumberFormat::DeDe, 0)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Double(Double::from_float(12.5)))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int16(expected)]]);
    }
    assert_eq!(
        run("SELECT StrComp(True,'true',1)", Some(&Host)).unwrap(),
        [vec![Value::Int16(0)]]
    );
}

#[test]
fn strcomp_scalar_conversion_keeps_mode_null_and_eager_failure_boundaries() {
    for (sql, expected) in [
        ("SELECT StrComp(1,'1',0)", EvalError::NumberFormatRequired),
        ("SELECT StrComp(1,2,3)", EvalError::Domain),
        (
            "SELECT StrComp(1,Null,Null)",
            EvalError::InvalidNullConversion,
        ),
        ("SELECT StrComp(Null,1/0,0)", EvalError::DivisionByZero),
    ] {
        assert_eq!(run(sql, None).err(), Some(QueryError::Evaluation(expected)));
    }
    assert_eq!(
        run("SELECT StrComp(1,Null,0)", None).unwrap(),
        [vec![Value::Null]]
    );
}
