use super::*;

#[test]
fn nullable_decimal_key_alias_retains_conservative_policy_and_first_scale() {
    let source = Data::decimal_fixture();
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let key = "IIf(True,D,A)";
    let expected = vec![
        vec![Value::Null, Value::Int64(2), Value::Boolean(true)],
        vec![decimal(100, 2), Value::Int64(2), Value::Boolean(false)],
    ];
    for having in ["", " HAVING Count(*)>=2"] {
        let sql = format!("SELECT {key} AS K,Count(*),(K&'x')='x' FROM T GROUP BY {key}{having}");
        for declared in [false, true] {
            let actual = stream(
                &source,
                &sql,
                ExecutionOptions {
                    collation: Some(&ORDINAL),
                    collation_resolver: Some(&resolver),
                    ..options(declared)
                },
            )
            .unwrap();
            assert_eq!(actual, expected);
        }
    }
    assert!(
        resolver.hits.get() > 0,
        "NULL provenance must not disappear"
    );
}

#[test]
fn normalized_decimal_groups_preserve_independent_late_string_cohorts() {
    let source = Data::new(
        ScalarType::Decimal,
        vec![
            (decimal(100, 2), 1, "15"),
            (decimal(20, 1), 4, "45"),
            (decimal(10, 1), 2, "15"),
            (decimal(200, 2), 5, "45"),
            (decimal(1, 0), 3, "15"),
            (decimal(2, 0), 6, "45"),
        ],
    );
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let expression = "IIf(N Mod 3=0,B,N)&IIf(True,0,A)";
    let sql = format!("SELECT D,Min({expression}),Max({expression}) FROM T GROUP BY D");
    let actual = stream(
        &source,
        &sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap();
    let expected = vec![
        vec![
            decimal(100, 2),
            Value::Text("20".into()),
            Value::Text("10".into()),
        ],
        vec![
            decimal(20, 1),
            Value::Text("50".into()),
            Value::Text("40".into()),
        ],
    ];
    assert_eq!(actual, expected);
    assert!(resolver.hits.get() > 0);
}

#[test]
fn fixed_grouping_does_not_hide_real_projection_conflicts_or_eager_operand_errors() {
    let source = Data::decimal_fixture();
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let error = stream(
        &source,
        "SELECT D,Min(IIf(N=2,A,B)) FROM T GROUP BY D",
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::CollationConflict)
    ));
    assert_eq!(resolver.hits.get(), 0);

    let error = stream(
        &source,
        "SELECT D,Sum(1/(N-3)) FROM T GROUP BY D",
        options(false),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}
