use super::*;

#[test]
fn field_free_projections_preserve_join_multiplicity_and_top() {
    let source = source();
    for expression in ["1", "(SELECT 1)", "IIf(True,1,2)"] {
        let sql =
            format!("SELECT DISTINCTROW {expression} AS V FROM A INNER JOIN B ON A.Id=B.Owner");
        assert_eq!(run(&sql, &source).unwrap(), vec![vec![Value::Int64(1)]; 7]);
    }
    assert_eq!(
        run(
            "SELECT DISTINCTROW TOP 2 1 AS V FROM A INNER JOIN B ON A.Id=B.Owner",
            &source
        )
        .unwrap(),
        vec![vec![Value::Int64(1)]; 2]
    );
    assert_eq!(
        run(
            "SELECT DISTINCT 1 AS V FROM A INNER JOIN B ON A.Id=B.Owner",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(1)]]
    );
}

#[test]
fn owner_free_counts_and_group_outputs_keep_all_joined_inputs() {
    let source = source();
    assert_eq!(
        run(
            "SELECT DISTINCTROW Count(*) FROM A INNER JOIN B ON A.Id=B.Owner",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(7)]]
    );
    assert_eq!(run("SELECT DISTINCTROW Count(*) AS N FROM A INNER JOIN B ON A.Id=B.Owner GROUP BY A.Hidden ORDER BY N",&source).unwrap(),[vec![Value::Int64(1)],vec![Value::Int64(2)],vec![Value::Int64(4)]]);
    assert_eq!(
        run(
            "SELECT DISTINCTROW 1 FROM A INNER JOIN B ON A.Id=B.Owner GROUP BY A.Hidden",
            &source
        )
        .unwrap(),
        vec![vec![Value::Int64(1)]; 3]
    );
    assert!(
        run(
            "SELECT DISTINCTROW 1 FROM A INNER JOIN B ON A.Id=B.Owner WHERE False",
            &source
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        run(
            "SELECT DISTINCTROW Count(*) FROM A INNER JOIN B ON A.Id=B.Owner WHERE False",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(0)]]
    );
}

#[test]
fn field_free_parameter_projections_keep_declared_carriers_and_eager_failures() {
    let source = source();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT DISTINCTROW P FROM A INNER JOIN B ON A.Id=B.Owner",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[("P", Value::Int32(42))],
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(result.rows(), vec![vec![Value::Int32(42)]; 7]);
    assert_eq!(
        run(
            "SELECT DISTINCTROW 1/0 FROM A INNER JOIN B ON A.Id=B.Owner",
            &source
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}
