use super::{domain_group_policy::TrimProvider, support::*};
use std::cell::Cell;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::QueryLimits;

#[test]
fn crosstab_cell_expressions_retain_the_complete_row_group_key_policy() {
    let source = Fixture::new();
    let key = r#"DLookup(Choose(Id,"'a'","'a '",'A',"'a'"),'T','Id=1')"#;
    for sql in [
        format!("SELECT IIf({key}='a ',Count(*),0) FROM T GROUP BY {key}"),
        format!(
            "TRANSFORM IIf({key}='a ',Count(*),0) SELECT 1 AS G FROM T GROUP BY {key} PIVOT 1 IN (1)"
        ),
    ] {
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimProvider),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows()[0].last(), Some(&Value::Int64(4)), "{sql}");
    }
}

#[test]
fn separate_pivot_buckets_share_group_provenance_but_not_aggregate_operand_frames() {
    let source = Fixture::new();
    let key = r#"DLookup(Choose(Id,"'a'","'a '",'A',"'a'"),'T','Id=1')"#;
    for (cell, expected) in [
        (format!("IIf({key}='a ',Count(*),0)"), 2),
        (format!("IIf((SELECT {key})='a ',Count(*),0)"), 2),
        (format!("Count(IIf({key}='a ',1,NULL))"), 1),
    ] {
        let sql = format!(
            "TRANSFORM {cell} SELECT 1 AS G FROM T GROUP BY {key} PIVOT IIf(Id<3,1,2) IN (1,2)"
        );
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimProvider),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Int64(1),
                Value::Int64(expected),
                Value::Int64(expected)
            ]],
            "{sql}"
        );
    }
}

#[test]
fn parameterized_named_cells_retain_final_group_policy_without_binding_scans() {
    let mut source = Fixture::new();
    let key = r#"DLookup(Choose(Id,P,"'a '",'A',P),'T','Id=1')"#;
    let definition = format!(
        concat!(
            "PARAMETERS P TEXT; TRANSFORM IIf({key}='a ',Count(*),0) ",
            "SELECT 1 AS G FROM T GROUP BY {key} PIVOT 1 IN (1)"
        ),
        key = key,
    );
    source.named = parse_query(&definition, ParseLimits::default())
        .unwrap()
        .into_query();
    let query = parse_query("SELECT [1] FROM Named", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    for (parameter, values) in [("'a'", vec![4]), ("'z'", vec![0, 2])] {
        let expected = values
            .into_iter()
            .map(|value| vec![Value::Int64(value)])
            .collect::<Vec<_>>();
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text(parameter.into()))],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimProvider),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            expected
        );
    }
}

#[test]
fn null_groups_and_conflicting_group_policies_keep_existing_outcomes() {
    let source = Fixture::new();
    let key = r#"DLookup(Choose(Id,"NULL","NULL",'A',"NULL"),'T','Id=1')"#;
    let sql = format!(
        "TRANSFORM IIf(IsNull({key}),Count(*),0) SELECT IsNull({key}) AS G FROM T GROUP BY {key} PIVOT 1 IN (1)"
    );
    let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Boolean(true), Value::Int64(3)],
            vec![Value::Boolean(false), Value::Int64(0)]
        ]
    );

    let key = r#"DLookup(Choose(Id,"'a'",'A','B',"'a'"),'T','Id=1')"#;
    for sql in [
        format!("SELECT IIf({key}='a',Count(*),0) FROM T GROUP BY {key}"),
        format!(
            "TRANSFORM IIf({key}='a',Count(*),0) SELECT 1 AS G FROM T GROUP BY {key} PIVOT 1 IN (1)"
        ),
    ] {
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&DEFAULT),
                        collation_resolver: Some(&Resolver::new()),
                        ..Default::default()
                    }
                )
                .err(),
            Some(QueryError::CollationConflict)
        );
    }
}

#[test]
fn explicit_global_overrides_and_required_only_providers_keep_grouped_select_parity() {
    let source = Fixture::new();
    let key = r#"DLookup(Choose(Id,"'a'","'a '",'A',"'a'"),'T','Id=1')"#;
    for (global, resolver) in [
        (Some(&DEFAULT as &dyn Collation), None),
        (None, Some(&TrimProvider as &dyn CollationResolver)),
    ] {
        let mut observed = Vec::new();
        for sql in [
            format!("SELECT IIf({key}='a ',Count(*),0) FROM T GROUP BY {key}"),
            format!(
                "TRANSFORM IIf({key}='a ',Count(*),0) SELECT 1 AS G FROM T GROUP BY {key} PIVOT 1 IN (1)"
            ),
        ] {
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            let result = prepare(parsed.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: global,
                        collation_resolver: resolver,
                        ..Default::default()
                    },
                )
                .unwrap();
            observed.push(
                result
                    .rows()
                    .iter()
                    .map(|row| row.last().unwrap().clone())
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(observed[0], observed[1]);
    }
}

#[test]
fn late_cell_failures_and_cancellation_do_not_publish_partial_crosstabs() {
    let source = Fixture::new();
    let key = r#"DLookup(Choose(Id,"'a'","'a '",'A',"'a'"),'T','Id=1')"#;
    let sql = format!(
        "TRANSFORM IIf({key}='a ',Count(*),0) SELECT 1 AS G FROM T GROUP BY {key} PIVOT IIf(Id<3,1,2) IN (1,2)"
    );
    let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    let before = source.scans.get();
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&TrimProvider),
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    // One parent scan, four group-key domain calls and one per bucket cell.
    assert_eq!(source.scans.get() - before, 7);
    let total = calls.get();
    for stop in [total / 2, total - 2] {
        calls.set(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimProvider),
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimProvider),
                    ..Default::default()
                }
            ),
            Err(QueryError::Limit(_))
        ));
    }
    for cell in [
        format!("IIf({key}='a ',Count(*),1/0)"),
        format!("IIf({key}='a ',Count(*),0)+Sum(1/(4-Id))"),
    ] {
        let sql = format!(
            "TRANSFORM {cell} SELECT 1 AS G FROM T GROUP BY {key} PIVOT IIf(Id<3,1,2) IN (1,2)"
        );
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&DEFAULT),
                        collation_resolver: Some(&TrimProvider),
                        ..Default::default()
                    }
                )
                .err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
}

#[test]
fn keys_include_unselected_pivot_members_while_fresh_text_results_keep_default_policy() {
    let source = Fixture::new();
    let key = r#"DLookup(Choose(Id,"'a'","'a '",'A',"'a'"),'T','Id=1')"#;
    for (condition, expected) in [(format!("{key}='a '"), 2), (format!("CStr({key})='a '"), 0)] {
        let sql = format!(
            concat!(
                "TRANSFORM IIf({condition},Count(*),0) SELECT 1 AS G FROM T ",
                "GROUP BY {key} PIVOT IIf(Id<3,1,2) IN (1)"
            ),
            condition = condition,
            key = key,
        );
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimProvider),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int64(1), Value::Int64(expected)]],
            "{sql}"
        );
    }
}
