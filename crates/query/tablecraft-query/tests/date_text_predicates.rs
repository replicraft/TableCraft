use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        dates::{
            TwoDigitYearWindow,
            parsing::{DateTextFormat, ReferenceYear},
        },
    },
};
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                Column::new("D".into(), ScalarType::DateTime, true).unwrap(),
                Column::new("Spelling".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![
                    Value::Int32(1),
                    Value::DateTime(LegacyDateTime::from_ole_days(45658.5).unwrap()),
                    Value::Text("2025-01-01 12:00:00".into()),
                ],
                vec![
                    Value::Int32(2),
                    Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
                    Value::Text("bad".into()),
                ],
                vec![Value::Int32(3), Value::Null, Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, options: ExecutionOptions<'_>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(&source, &[], options)?
        .rows()
        .to_vec())
}
fn iso() -> ExecutionOptions<'static> {
    ExecutionOptions {
        date_format: Some(DateTextFormat::Iso),
        ..Default::default()
    }
}

#[test]
fn date_text_filters_membership_domains_and_nested_eval_share_conversion() {
    for sql in [
        "SELECT ID FROM T WHERE D>'2025-01-01'",
        "SELECT ID FROM T WHERE D BETWEEN '2025-01-01' AND '2025-01-02'",
        "SELECT ID FROM T WHERE D IN ('2025-01-01 12:00:00')",
        "SELECT ID FROM T WHERE D IN (SELECT '2025-01-01 12:00:00')",
        "SELECT ID FROM T WHERE D=ANY (SELECT '2025-01-01 12:00:00')",
        "SELECT T.ID FROM T INNER JOIN (SELECT '2025-01-01 12:00:00' AS S) AS X ON T.D=X.S",
    ] {
        assert_eq!(run(sql, iso()).unwrap(), [vec![Value::Int32(1)]], "{sql}");
    }
    assert_eq!(
        run(
            "SELECT DCount('*','T',\"D>'2025-01-01'\"),Eval(\"#2025-01-01#='2025-01-01'\")",
            iso()
        )
        .unwrap(),
        [vec![Value::Int64(1), Value::Boolean(true)]]
    );
    assert_eq!(
        run(
            "SELECT ID FROM T WHERE D='-1,25'",
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..iso()
            }
        )
        .unwrap(),
        [vec![Value::Int32(2)]]
    );
}

#[test]
fn prepared_date_predicates_use_current_year_and_date_policies() {
    let source = source();
    let syntax = parse_query(
        "PARAMETERS P TEXT; SELECT #2030-01-01#=P",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (max, expected) in [(2029, false), (2099, true)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text("1/1/30".into()))],
                ExecutionOptions {
                    date_format: Some(DateTextFormat::EnUs),
                    year_window: Some(TwoDigitYearWindow::new(max).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Boolean(expected)]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Text("1/1".into()))],
            ExecutionOptions {
                date_format: Some(DateTextFormat::EnUs),
                reference_year: Some(ReferenceYear::new(2030).unwrap()),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![Value::Boolean(true)]]
    );
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Text("2030-01-01".into()))],
            Default::default()
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DateFormatRequired))
    );
}

#[test]
fn date_predicates_keep_eager_failures_and_strict_key_boundaries() {
    assert_eq!(
        run("SELECT ID FROM T WHERE ID=1 AND D=Spelling", iso()).err(),
        Some(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    assert_eq!(
        run("SELECT #2025-01-01# IN ('2025-01-01',1/0)", iso()).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT Null='bad'", Default::default()).unwrap(),
        [vec![Value::Null]]
    );
    assert!(
        run(
            "SELECT DISTINCT X FROM (SELECT #2025-01-01# AS X UNION ALL SELECT '2025-01-01') AS Q",
            iso()
        )
        .is_err()
    );
    assert!(run("SELECT #2025-01-01#='bad'", iso()).is_err());
}
