use tablecraft_core::values::{Double, Value};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn prefix_calls_bind_group_and_rebind_text_parameters() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT Val(P) AS V,Count(*) FROM (SELECT 1 AS N) AS T GROUP BY Val(P)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (text, expected) in [("12cats", 12.0), ("2 4", 24.0), ("x", 0.0)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text(text.into()))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            [vec![
                Value::Double(Double::from_float(expected)),
                Value::Int64(1)
            ]]
        );
    }
    let query = parse_query("SELECT Sum(Val(S)) FROM (SELECT '12cats' AS S UNION ALL SELECT '2 4' AS S) AS T WHERE Val(S)>0", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Double(Double::from_float(36.0))]]
    );
}
#[test]
fn prefix_errors_and_unsupported_call_shapes_remain_visible() {
    let source = MemorySource::default();
    for sql in [
        "SELECT IIf(False,Val('50.5%'),0)",
        "SELECT Val(NULL)",
        "SELECT Val(1)",
        "SELECT Val()",
        "SELECT VBA.Val('1')",
        "SELECT Val('1',1/0)",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            prepare(query.query(), &source)
                .and_then(|plan| plan.execute(&source, &[], ExecutionOptions::default()))
                .is_err(),
            "{sql}"
        );
    }
}
