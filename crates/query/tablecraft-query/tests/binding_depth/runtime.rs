use super::support::{Data, Lexical, prepared, wrapped};
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{BindingDepth, ExecutionOptions, QueryError, QueryLimits};

#[test]
fn deep_text_policy_and_extrema_walks_use_the_captured_limit() {
    let source = Data::new(&[]);
    let expression = wrapped("S");
    let options = ExecutionOptions {
        collation: Some(&Lexical),
        collation_resolver: Some(&Lexical),
        ..Default::default()
    };
    let minimum = prepared(&format!("SELECT Min({expression}) FROM Words"), &source)
        .execute(&source, &[], options)
        .unwrap();
    assert_eq!(minimum.rows(), &[vec![Value::Text("a".into())]]);

    let groups = prepared(
        &format!("SELECT {expression} AS S,Count(*) FROM Words GROUP BY {expression} ORDER BY S"),
        &source,
    )
    .execute(&source, &[], options)
    .unwrap();
    assert_eq!(
        groups.rows(),
        &[
            vec![Value::Text("a".into()), Value::Int64(1)],
            vec![Value::Text("b".into()), Value::Int64(1)]
        ]
    );
}

#[test]
fn progressive_group_identity_and_admission_use_the_prepared_limit() {
    let source = Data::new(&[]);
    let expression = wrapped("K");
    let plan = prepared(
        &format!("SELECT {expression} AS K,Count(*) FROM T GROUP BY {expression}"),
        &source,
    );
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                binding_depth: BindingDepth::new(1).unwrap(),
                ..Default::default()
            },
            &mut |row| {
                rows.push(row.to_vec());
                Ok::<(), ()>(())
            },
        )
        .unwrap();
    assert_eq!(
        rows,
        [
            vec![Value::Int32(2), Value::Int64(1)],
            vec![Value::Int32(1), Value::Int64(1)],
            vec![Value::Int32(3), Value::Int64(1)]
        ]
    );
    assert_eq!(summary.scanned_rows, 3);
    assert_eq!(summary.emitted_rows, 3);
    assert_eq!(source.scans.get(), 0);
    assert_eq!(source.streams.get(), 1);
}

#[test]
fn deeper_plans_keep_work_limits_and_every_observed_cancellation_checkpoint() {
    let source = Data::new(&[]);
    let plan = prepared(&format!("SELECT {} FROM T", wrapped("K")), &source);
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    steps: 1,
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        Err(QueryError::Limit(_))
    ));
    assert_eq!(source.scans.get(), 0);

    let visits = Cell::new(0);
    let count = || {
        visits.set(visits.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    let total = visits.get();
    for boundary in 1..=total {
        visits.set(0);
        let cancelled = || {
            visits.set(visits.get() + 1);
            visits.get() == boundary
        };
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        cancelled: Some(&cancelled),
                        ..Default::default()
                    }
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {boundary}"
        );
    }
}
