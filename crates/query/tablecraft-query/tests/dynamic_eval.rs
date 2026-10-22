use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Formula".into(), ScalarType::Text, false).unwrap(),
            ],
            vec![
                vec![Value::Int32(1), Value::Text("15+3".into())],
                vec![Value::Int32(2), Value::Text("3*6".into())],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn dynamic_arguments_retain_field_parameter_group_aggregate_and_correlated_dependencies() {
    let source = source();
    for sql in [
        "SELECT Eval(Formula) FROM Items ORDER BY ID",
        "SELECT Eval(Formula) FROM Items GROUP BY Eval(Formula)",
        "SELECT Eval(First(Formula)) FROM Items",
        "SELECT Eval('15+3') FROM Items GROUP BY Eval('15+3')",
        "SELECT (SELECT Eval(A.Formula)) FROM Items AS A ORDER BY ID",
        "SELECT Eval(CStr(ID) & '+17') FROM Items WHERE ID=1",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let result = prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::Invariant),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows()[0][0], Value::Int64(18), "{sql}");
    }
    let query = parse_query("PARAMETERS P TEXT; SELECT Eval(P)", Default::default()).unwrap();
    let prepared = prepare(query.query(), &source).unwrap();
    for (input, expected) in [("1+1", 2), ("15+3", 18)] {
        assert_eq!(
            prepared
                .execute(
                    &source,
                    &[("P", Value::Text(input.into()))],
                    Default::default()
                )
                .unwrap()
                .rows(),
            [vec![Value::Int64(expected)]]
        );
    }
    let query = parse_query(
        "SELECT Eval('CVErr(2001)'),IsError(Eval('CVErr(2001)'))",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(query.query(), &source).unwrap();
    assert_eq!(
        prepared
            .execute_expression(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![
            ExpressionValue::Error(2001),
            ExpressionValue::Value(Value::Boolean(true))
        ]]
    );
    assert_eq!(
        prepared.execute(&source, &[], Default::default()).err(),
        Some(EvalError::NonScalarExpression.into())
    );
}

#[test]
fn dynamic_names_do_not_capture_source_fields_parameters_or_domains() {
    let source = source();
    for sql in [
        "SELECT Eval('ID') FROM Items",
        "PARAMETERS ID LONG; SELECT Eval('ID') FROM Items",
        "SELECT Eval('IIf(False,ID,1)') FROM Items",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let parameters = if sql.starts_with("PARAMETERS") {
            vec![("ID", Value::Int32(1))]
        } else {
            vec![]
        };
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &parameters, Default::default())
                .err(),
            Some(EvalError::UnboundField.into())
        );
    }
    for sql in [
        "SELECT Eval('DSum(''ID'',''Items'')')",
        "SELECT Eval('(SELECT 1)')",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(EvalError::UnsupportedExpression.into())
        );
    }
    let query = parse_query(
        "SELECT ID FROM Items WHERE ID=0 AND Eval('1/0')=0",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(EvalError::DivisionByZero.into())
    );
}

#[test]
fn grouping_and_pivots_do_not_hide_volatile_calls_inside_eval_strings() {
    let source = source();
    for sql in [
        "SELECT Count(*) FROM Items GROUP BY Eval('Rnd()')",
        "SELECT Count(*) FROM Items GROUP BY Eval('Eval(''Rnd()'')')",
        "SELECT Count(*) FROM Items GROUP BY Eval('Date()')",
        "TRANSFORM Count(*) SELECT ID FROM Items GROUP BY ID PIVOT Eval('Rnd()')",
        "TRANSFORM Count(*) SELECT ID FROM Items GROUP BY ID PIVOT Eval('Eval(''Rnd()'')')",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let state = RndState::default();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        random: Some(&state),
                        ..Default::default()
                    }
                )
                .err(),
            Some(EvalError::DynamicNotRepeatable.into()),
            "{sql}"
        );
        assert_eq!(state.apply(None), RndState::default().apply(None));
    }
    let query = parse_query("SELECT Eval('Rnd()'),Eval('Rnd(0)')", Default::default()).unwrap();
    let state = RndState::default();
    let reference = RndState::default().apply(None).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    random: Some(&state),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        [vec![reference.clone(), reference]]
    );
}

#[test]
fn parsing_and_execution_share_all_cancellation_points_and_quotas() {
    let source = source();
    let query = parse_query(
        "SELECT Eval(Formula) FROM Items GROUP BY Eval(Formula)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
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
    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    for limits in [
        QueryLimits {
            bytes: 100,
            ..Default::default()
        },
        QueryLimits {
            steps: 10,
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
}
