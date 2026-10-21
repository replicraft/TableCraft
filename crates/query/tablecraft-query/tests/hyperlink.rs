use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

struct LabelOrder;

impl Collation for LabelOrder {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&LabelOrder),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn hyperlink_components_retain_grouping_filters_correlations_and_aggregates() {
    assert_eq!(
        run("SELECT HyperlinkPart(A.H,2),Count(*) FROM (SELECT 'one#address#sub#tip' AS H UNION ALL SELECT 'two#address#other#tip') AS A GROUP BY HyperlinkPart(A.H,2)").unwrap(),
        [vec![Value::Text("address".into()), Value::Int64(2)]]
    );
    assert_eq!(
        run("SELECT (SELECT HyperlinkPart(A.H,3)) FROM (SELECT 'name#address#sub#tip' AS H) AS A WHERE HyperlinkPart(A.H,2)='address'").unwrap(),
        [vec![Value::Text("sub".into())]]
    );
    assert_eq!(
        run("SELECT HyperlinkPart(First(A.H),4) FROM (SELECT 'name#address#sub#tip' AS H) AS A")
            .unwrap(),
        [vec![Value::Text("tip".into())]]
    );
    assert_eq!(
        run("SELECT HyperlinkPart(A.H,2) FROM (SELECT 'x#y#z' AS H) AS A GROUP BY HyperlinkPart(A.H,3)").err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn required_selector_is_rebound_and_invalid_calls_do_not_hide_eager_failures() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT HyperlinkPart('name#address#sub#tip',P)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (selector, expected) in [(0, "name"), (2, "address"), (5, "address#sub")] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Int32(selector))],
                Default::default(),
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Text(expected.into())]]);
    }

    for sql in [
        "SELECT HyperlinkPart(Null,1/0)",
        "SELECT IIf(False,HyperlinkPart('x',1/0),'ignored')",
    ] {
        assert_eq!(run(sql).err(), Some(EvalError::DivisionByZero.into()));
    }
    for sql in [
        "SELECT HyperlinkPart('x')",
        "SELECT HyperlinkPart('x',)",
        "SELECT HyperlinkPart('x',2,1/0)",
        "SELECT HyperlinkPart(*)",
        "SELECT VBA.HyperlinkPart('x',2)",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported));
    }
}
