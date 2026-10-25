use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                declared_integral: declared,
                ..Default::default()
            },
        )
        .map(|result| result.rows().to_vec())
}

#[test]
fn retained_controls_preserve_selected_carriers_through_bound_and_derived_queries() {
    for declared in [false, true] {
        assert_eq!(
            run("SELECT IIf(ChrW(49),CInt(11),CInt(22)),Switch(ChrW(49),CInt(11),True,CInt(22)),Choose(ChrW(50),CInt(11),CInt(22))", declared).unwrap(),
            [vec![Value::Int16(11), Value::Int16(11), Value::Int16(22)]],
        );
        assert_eq!(
            run("SELECT IIf(C,A,B),Switch(C,A,True,B),Choose(C,A,B) FROM (SELECT ChrW(49) AS C,CInt(11) AS A,CInt(22) AS B) Q", declared).unwrap(),
            [vec![Value::Int16(11); 3]],
        );
        assert_eq!(
            run("SELECT Sum(IIf(C,CInt(1),CInt(2))) FROM (SELECT ChrW(49) AS C UNION ALL SELECT ChrW(48)) Q", declared).unwrap(),
            [vec![Value::Int64(3)]],
        );
    }
}

#[test]
fn semantic_raw_control_errors_do_not_hide_eager_projection_or_filter_failures() {
    for declared in [false, true] {
        for control in ["ChrW(55296)", "ChrB(49)", "'bad'"] {
            for expression in [
                format!("IIf({control},1/0,0)"),
                format!("Switch({control},1/0,True,0)"),
                format!("Choose({control},1/0,0)"),
            ] {
                for sql in [
                    format!("SELECT {expression}"),
                    format!("SELECT 1 FROM (SELECT 1 AS N) Q WHERE {expression}"),
                ] {
                    assert_eq!(
                        run(&sql, declared),
                        Err(EvalError::DivisionByZero.into()),
                        "{sql}"
                    );
                }
            }
        }
        assert_eq!(
            run(
                "SELECT IIf(C,1/0,0) FROM (SELECT ChrW(55296) AS C) Q",
                declared
            ),
            Err(EvalError::DivisionByZero.into()),
        );
    }
}

#[test]
fn retained_control_conversion_remains_strict_and_needs_an_explicit_number_policy() {
    for sql in [
        "SELECT IIf(ChrW(55296),1,2)",
        "SELECT Switch(ChrW(55296),1,True,2)",
        "SELECT Choose(ChrW(55296),1,2)",
    ] {
        assert_eq!(
            run(sql, false),
            Err(EvalError::UnrepresentableText.into()),
            "{sql}"
        );
    }
    let source = MemorySource::default();
    for sql in [
        "SELECT IIf(ChrW(49),1,2)",
        "SELECT Switch(ChrW(49),1,True,2)",
        "SELECT Choose(ChrW(49),1,2)",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(EvalError::NumberFormatRequired.into()),
            "{sql}",
        );
    }
}

#[test]
fn typed_parameters_feed_retained_controls_without_retyping_branch_outputs() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "PARAMETERS P Text; SELECT IIf(LeftB(P,2),CInt(7),CInt(8)),Choose(LeftB(P,2),CInt(7),CInt(8))",
        Default::default(),
    ).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (text, expected) in [("1", [7, 7]), ("2", [7, 8])] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(text.into()))],
                ExecutionOptions {
                    number_format: Some(NumberFormat::Invariant),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [expected.map(Value::Int16).to_vec()]);
    }
}

#[test]
fn control_publication_and_eager_branches_share_query_quotas_and_cancellation() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "SELECT IIf(ChrW(49),N,N+1),Switch(ChrW(49),N,True,N+1),Choose(ChrW(49),N,N+1) FROM (SELECT 1 AS N UNION ALL SELECT 2) Q",
        Default::default(),
    ).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            number_format: Some(NumberFormat::Invariant),
            cancelled: Some(&observe),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(NumberFormat::Invariant),
                        cancelled: Some(&cancel),
                        ..Default::default()
                    },
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                limits: QueryLimits {
                    bytes: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        Err(QueryError::Limit(_))
    ));
}
