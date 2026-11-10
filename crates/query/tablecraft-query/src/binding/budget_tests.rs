use super::{StageOptions, prepare, prepare_stage, test_support::NoSource};
use crate::{QueryError, source::MemorySource};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_sql::{
    parse_query,
    query::{Query, ReadBody, Statement},
    select::{Expression, Projection},
};

fn query(text: String) -> Query {
    let parsed = parse_query("SELECT 0 AS S", Default::default()).unwrap();
    let mut query = parsed.into_query();
    let Statement::Read(read) = &mut query.statement else {
        panic!()
    };
    let ReadBody::Select(select) = &mut read.body else {
        panic!()
    };
    let Projection::Expression { expression, .. } = &mut select.projection[0] else {
        panic!()
    };
    *expression = Expression::Text(text);
    query
}

#[test]
fn retained_literal_above_one_mib_remains_owned_after_syntax_is_dropped() {
    let text = "é🦀\0".repeat(200_000);
    assert!(text.len() > 1_048_576);
    let query = query(text.clone());
    let plan = prepare(&query, &NoSource).unwrap();
    drop(query);
    let rows = plan
        .execute(&MemorySource::default(), &[], Default::default())
        .unwrap();
    assert_eq!(rows.rows(), &[vec![Value::Text(text)]]);
}

#[test]
fn retained_binding_bytes_spend_only_explicit_caller_allowances_and_cancel_before_copy() {
    let query = query("x".repeat(1_048_577));
    let mut exact = ResourceBudget::new(2 * 1024 * 1024, 0, 10_000);
    assert!(
        prepare_stage(
            &query,
            &NoSource,
            &[],
            Some(&mut exact),
            None,
            StageOptions::default()
        )
        .unwrap()
        .is_ok()
    );
    let mut small = ResourceBudget::new(1_048_576, 0, 10_000);
    assert!(matches!(
        prepare_stage(
            &query,
            &NoSource,
            &[],
            Some(&mut small),
            None,
            StageOptions::default()
        ),
        Err(QueryError::Limit(_))
    ));
    let mut allowance = ResourceBudget::new(2 * 1024 * 1024, 0, 10_000);
    assert!(matches!(
        prepare_stage(
            &query,
            &NoSource,
            &[],
            Some(&mut allowance),
            Some(&|| true),
            StageOptions::default()
        ),
        Err(QueryError::Cancelled)
    ));
}
