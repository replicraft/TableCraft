use super::super::InsertError;
use super::{Target, budget, query};
use crate::{MutationOptions, QueryError, plan_insert, plan_insert_with_options};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_sql::{
    query::{InsertSource, Statement},
    select::Expression,
};

#[test]
fn optioned_insert_pays_actual_binding_allocations_instead_of_a_fixed_reservation() {
    let query = query("INSERT INTO Items VALUES(1,'x')");
    let calls = Cell::new(0);
    let result = plan_insert_with_options(
        Target::ordinary(&calls),
        query.query(),
        &mut ResourceBudget::new(16_384, 100, 100_000),
        &mut budget(),
        MutationOptions::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result, [Value::Int32(1), Value::Text("x".into())]);
    assert_eq!(calls.get(), 1);
}

#[test]
fn large_values_syntax_and_optioned_binding_honor_actual_caller_budgets() {
    let mut query = query("INSERT INTO Items VALUES(1,'x')").into_query();
    let Statement::Insert(insert) = &mut query.statement else {
        panic!()
    };
    let InsertSource::Values(values) = &mut insert.source else {
        panic!()
    };
    values[1] = Expression::Text("x".repeat(1_048_577));
    for optioned in [false, true] {
        let calls = Cell::new(0);
        let mut allowance = ResourceBudget::new(16 * 1024 * 1024, 100, 10_000_000);
        let result = if optioned {
            plan_insert_with_options(
                Target::ordinary(&calls),
                &query,
                &mut allowance,
                &mut budget(),
                MutationOptions::default(),
                &|| false,
            )
        } else {
            plan_insert(Target::ordinary(&calls), &query, &mut allowance, &|| false)
        }
        .unwrap();
        assert_eq!(result[0], Value::Int32(1));
        let Value::Text(text) = &result[1] else {
            panic!()
        };
        assert_eq!(text.len(), 1_048_577);
        assert!(text.bytes().all(|byte| byte == b'x'));
        assert_eq!(calls.get(), 1);
    }
    let calls = Cell::new(0);
    assert!(matches!(
        plan_insert_with_options(
            Target::ordinary(&calls),
            &query,
            &mut ResourceBudget::new(1_048_576, 100, 10_000_000),
            &mut budget(),
            MutationOptions::default(),
            &|| false
        ),
        Err(InsertError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(calls.get(), 0);
}
