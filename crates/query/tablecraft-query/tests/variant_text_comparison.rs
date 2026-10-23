use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(
    sql: &str,
    declared: bool,
    format: Option<NumberFormat>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn both_variant_numeric_and_text_inputs_use_category_order_for_all_comparisons() {
    for number in [
        "CByte(255)",
        "CInt(-32768)",
        "CLng(2147483647)",
        "CLngLng(9223372036854775807)",
        "CSng(-0.0)",
        "CDbl(1.7976931348623157e308)",
        "CCur(1.25)",
        "CDec(0.1)",
    ] {
        for text in ["''", "'1'", "'not numeric'"] {
            for (operator, forward, reverse) in [
                ("=", false, false),
                ("<>", true, true),
                ("<", true, false),
                ("<=", true, false),
                (">", false, true),
                (">=", false, true),
            ] {
                for (left, right, expected) in [(number, text, forward), (text, number, reverse)] {
                    let sql = format!("SELECT CVar({left}){operator}CVar({right})");
                    assert_eq!(
                        run(&sql, true, None).unwrap(),
                        vec![vec![Value::Boolean(expected)]],
                        "{sql}"
                    );
                }
            }
        }
    }
}

#[test]
fn ordinary_one_variant_unknown_and_boolean_pairs_keep_their_conversion_profiles() {
    let sql = "SELECT CVar(1)=CVar('1')";
    assert_eq!(
        run(sql, true, None).unwrap(),
        vec![vec![Value::Boolean(false)]]
    );
    assert_eq!(
        run(sql, false, Some(NumberFormat::EnUs)).unwrap(),
        vec![vec![Value::Boolean(true)]]
    );
    for sql in [
        "SELECT CVar(1)='1'",
        "SELECT 1=CVar('1')",
        "SELECT 1='1'",
        "SELECT CVar(True)=CVar('-1')",
        "SELECT N=CVar('1') FROM (SELECT CVar(1) AS N UNION ALL SELECT CVar(2)) AS Q",
    ] {
        assert_eq!(
            run(sql, true, None),
            Err(EvalError::NumberFormatRequired.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT CVar(1)='1'", true, Some(NumberFormat::EnUs)).unwrap(),
        vec![vec![Value::Boolean(true)]]
    );
    assert_eq!(
        run("SELECT CVar(NULL)<CVar('bad')", true, None).unwrap(),
        vec![vec![Value::Null]]
    );
    assert_eq!(
        run("SELECT CVar(1)<CVar(CStr(1/0))", true, None),
        Err(EvalError::DivisionByZero.into())
    );
}

#[test]
fn central_policy_covers_filters_joins_and_membership_without_reinterpreting_keys() {
    for sql in [
        "SELECT N<CVar('bad') FROM (SELECT CVar(1) AS N) AS Q",
        "SELECT (SELECT Q.N<CVar('bad')) FROM (SELECT CVar(1) AS N) AS Q",
        "SELECT N<CVar('bad') FROM (SELECT CVar(1) AS N) AS Q GROUP BY N",
        "SELECT CVar(1)<ALL(SELECT CVar('bad'))",
        "SELECT CVar(1)<ANY(SELECT CVar('bad'))",
    ] {
        assert_eq!(
            run(sql, true, None).unwrap(),
            vec![vec![Value::Boolean(true)]],
            "{sql}"
        );
    }
    for sql in [
        "SELECT CVar(1) IN (CVar('1'),CVar('bad'))",
        "SELECT CVar(1) IN (SELECT CVar('1'))",
        "SELECT CVar(1) BETWEEN CVar('0') AND CVar('2')",
    ] {
        assert_eq!(
            run(sql, true, None).unwrap(),
            vec![vec![Value::Boolean(false)]],
            "{sql}"
        );
    }
    let sql = "SELECT Count(*) FROM (SELECT CVar(1) AS N) AS Q WHERE N<CVar('bad')";
    assert_eq!(run(sql, true, None).unwrap(), vec![vec![Value::Int64(1)]]);
    let sql = "SELECT Count(*) FROM (SELECT CVar(1) AS N) AS Q HAVING CVar(Count(*))<CVar('bad')";
    assert_eq!(run(sql, true, None).unwrap(), vec![vec![Value::Int64(1)]]);
    let sql = "SELECT A.N FROM (SELECT CVar(1) AS N) AS A INNER JOIN (SELECT CVar('1') AS S) AS B ON A.N=B.S";
    assert!(run(sql, true, None).unwrap().is_empty());
    assert_eq!(
        run(sql, false, Some(NumberFormat::EnUs)).unwrap(),
        vec![vec![Value::Int64(1)]]
    );
}

struct RejectCollation(Cell<usize>);

impl Collation for RejectCollation {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        self.0.set(self.0.get() + 1);
        Err(EvalError::Domain)
    }
}

#[test]
fn category_ordering_needs_no_text_provider_and_remains_cancellable() {
    let source = MemorySource::default();
    let collation = RejectCollation(Cell::new(0));
    let parsed = parse_query("SELECT CVar(1)<CVar('bad')", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let options = ExecutionOptions {
        declared_integral: true,
        collation: Some(&collation),
        ..Default::default()
    };
    assert_eq!(
        plan.execute(&source, &[], options).unwrap().rows(),
        &[vec![Value::Boolean(true)]]
    );
    assert_eq!(collation.0.get(), 0);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| true),
                ..options
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
    let parsed = parse_query("SELECT CVar('a')=CVar('a')", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options)
            .err(),
        Some(EvalError::Domain.into())
    );
    assert_eq!(collation.0.get(), 1);
}
