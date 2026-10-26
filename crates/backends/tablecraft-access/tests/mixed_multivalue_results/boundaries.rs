use super::*;
use tablecraft_expressions::EvalError;
use tablecraft_query::QueryLimits;

#[test]
fn native_selected_corruption_eager_scalar_errors_and_limits_never_return_a_prefix() {
    let source = NativeSource::new(
        Cursor::new(join_fixture::database(6, 0)),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let query = parse_query(
        "SELECT TOP 1 Score,10 \\ (Score.Value-200) AS Boom FROM Items",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(
        plan.execute_collections(&source, &[], Default::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    let query = parse_query("SELECT Score,Score.Value FROM Items", Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let result = plan.execute_collections(
        &source,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                rows: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(matches!(result.err(), Some(QueryError::Limit(_))));

    let mut bytes = join_fixture::database(6, 0);
    bytes[CHILD_DATA_PAGE * PAGE_BYTES] = 0;
    let source = NativeSource::new(
        Cursor::new(bytes),
        CatalogScope::new(2),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let plan = prepare(query.query(), &source).unwrap();
    assert!(
        plan.execute_collections(&source, &[], Default::default())
            .is_err()
    );
}
