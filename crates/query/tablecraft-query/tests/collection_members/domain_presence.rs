use super::*;
use tablecraft_core::values::Double;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::QueryLimits;

#[test]
fn all_domain_aggregates_filter_parent_membership_without_expanding_children() {
    let source = Attachments::new();
    let sql = "SELECT DCount('*','Items','Files IS NOT NULL'),DCount('Id','Items','Files IS NULL'),DSum('Id','Items','Files IS NOT NULL'),DAvg('Id','Items','Files IS NOT NULL'),DMin('Id','Items','Files IS NOT NULL'),DMax('Id','Items','Files IS NOT NULL'),DFirst('Id','Items','Files IS NOT NULL'),DLast('Id','Items','Files IS NOT NULL'),DLookup('Id','Items','Files IS NULL'),DVar('Id','Items','Files IS NOT NULL'),DVarP('Id','Items','Files IS NOT NULL'),DStDev('Id','Items','Files IS NOT NULL'),DStDevP('Id','Items','Files IS NOT NULL')";
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    let expected = vec![
        Value::Int64(2),
        Value::Int64(1),
        Value::Int64(4),
        Value::Double(Double::from_float(2.0)),
        Value::Int32(1),
        Value::Int32(3),
        Value::Int32(1),
        Value::Int32(3),
        Value::Int32(2),
        Value::Double(Double::from_float(2.0)),
        Value::Double(Double::from_float(1.0)),
        Value::Double(Double::from_float(2_f64.sqrt())),
        Value::Double(Double::from_float(1.0)),
    ];
    assert_eq!(result.rows().len(), 1);
    assert_eq!(result.rows()[0].len(), expected.len());
    for (actual, expected) in result.rows()[0].iter().zip(expected) {
        if let (Value::Double(actual), Value::Double(expected)) = (actual, &expected) {
            assert!((actual.to_float() - expected.to_float()).abs() < 1e-12);
        } else {
            assert_eq!(*actual, expected);
        }
    }
}

#[test]
fn dynamic_domain_criteria_are_request_local_and_nested_calls_retain_scalar_scope() {
    let source = Attachments::new();
    let query = parse_query(
        "PARAMETERS Criteria TEXT; SELECT DCount('*','Items',Criteria)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (criteria, expected) in [
        ("Files IS NULL", Value::Int64(1)),
        ("Files IS NOT NULL", Value::Int64(2)),
        ("Missing IS NULL", Value::Null),
        ("NOT (Files IS NULL) AND Id>1", Value::Int64(1)),
        ("Files IS NULL", Value::Int64(1)),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("Criteria", text(criteria))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            [vec![expected]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("Criteria", text("Files.FileName IS NULL"))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(
        plan.execute(
            &source,
            &[("Criteria", text("Files IS NOT NULL"))],
            ExecutionOptions::default()
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(2)]]
    );

    let nested = r#"SELECT DCount('*','Items',"Files IS NOT NULL AND Id=DLookup('Id','Items','Files IS NOT NULL AND Id=3')")"#;
    assert_eq!(
        run(nested, &source).unwrap().rows(),
        [vec![Value::Int64(1)]]
    );
    let query = parse_query(
        "SELECT Id,DCount('*','Items','Files IS NOT NULL AND Id=' & Id) FROM Items ORDER BY Id",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), Value::Int64(1)],
            vec![Value::Int32(2), Value::Int64(0)],
            vec![Value::Int32(3), Value::Int64(1)],
        ]
    );
}

#[test]
fn domain_presence_preserves_eager_complete_reads_and_rejects_member_value_scope() {
    let source = Attachments::new();
    for sql in [
        "SELECT DCount('*','Items','Files IS NULL OR 1/0')",
        "SELECT DCount('*','Items','Files.FileName IS NULL')",
        "SELECT DCount('Files','Items','Files IS NULL')",
        "SELECT DCount('*','Items','IIf(Files IS NULL,True,False)')",
    ] {
        assert!(run(sql, &source).is_err(), "{sql}");
    }

    let mut source = Attachments::new();
    source.lists[2][0] = vec![Value::Int32(42)];
    source.ignore_errors = true;
    for sql in [
        "SELECT DLookup('Id','Items','Files IS NOT NULL')",
        "SELECT DCount('*','Items','True OR Files IS NULL')",
    ] {
        assert!(run(sql, &source).is_err(), "{sql}");
    }
}

#[test]
fn domain_presence_reuses_cumulative_limits_and_all_cancellation_checkpoints() {
    let source = Attachments::new();
    let query = parse_query(
        "SELECT DCount('*','Items','Files IS NOT NULL'),DCount('*','Items','Files IS NULL')",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for limits in [
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }

    let checks = Cell::new(0);
    let invoke = |stop| {
        checks.set(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() == stop
        };
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    assert!(invoke(usize::MAX).is_ok());
    let total = checks.get();
    for stop in 1..=total {
        assert!(
            matches!(
                invoke(stop).err(),
                Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
}
