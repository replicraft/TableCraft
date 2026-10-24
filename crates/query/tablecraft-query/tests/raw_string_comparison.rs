use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn raw_comparison_options_and_scalar_spellings_use_explicit_numeric_policies() {
    let source = MemorySource::default();
    let parsed = parse_query("SELECT StrComp(ChrW(49),1,'0,5')", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int16(0)]]
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}

#[test]
fn raw_binary_comparison_composes_through_grouping_and_correlations() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT StrComp(ChrW(N),ChrW(55296),0) AS K,Count(*),(SELECT StrComp(ChrW(Q.N),ChrW(55296),0)) AS C FROM (SELECT 55296 AS N UNION ALL SELECT 57343) AS Q GROUP BY StrComp(ChrW(N),ChrW(55296),0) ORDER BY K",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[
            vec![Value::Int16(0), Value::Int64(1), Value::Int16(0)],
            vec![Value::Int16(1), Value::Int64(1), Value::Int16(1)],
        ]
    );
}

#[test]
fn raw_comparison_modes_and_nulls_retain_admission_and_eager_failures() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("StrComp(ChrB(65),NULL,3)", EvalError::Domain),
        (
            "StrComp(ChrW(55296),ChrW(55296),NULL)",
            EvalError::InvalidNullConversion,
        ),
        (
            "StrComp(ChrB(65),ChrB(65),0)",
            EvalError::UnrepresentableText,
        ),
        ("StrComp(ChrW(55296),'x',1)", EvalError::UnrepresentableText),
        ("StrComp(ChrW(65536),NULL,0)", EvalError::Domain),
        ("StrComp(ChrW(55296),NULL,1/0)", EvalError::DivisionByZero),
    ] {
        let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{expression}"
        );
    }
}

#[test]
fn raw_comparisons_rebind_and_cancel_at_every_observed_checkpoint() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT StrComp(ChrW(P),ChrW(55296),0),StrComp(ChrW(P),NULL,0)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (code, order) in [(65, -1), (55296, 0), (57343, 1)] {
        assert_eq!(
            plan.execute(&source, &[("P", Value::Int32(code))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int16(order), Value::Null]]
        );
    }

    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[("P", Value::Int32(55296))],
        ExecutionOptions {
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
        assert!(matches!(
            plan.execute(
                &source,
                &[("P", Value::Int32(55296))],
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
