use super::*;

#[test]
fn null_key_bridge_preserves_conservative_text_policy_and_null_carrier() {
    let source = Data::new(&[(2, 1, "first"), (1, 2, "second")]);
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let sql = "SELECT IIf(True,NULL,A) AS K,Count(*),K IS NULL,(K&'x')='x' \
               FROM T GROUP BY IIf(True,NULL,A)";
    for declared in [false, true] {
        let actual = stream(
            &source,
            sql,
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..options(declared)
            },
        )
        .unwrap();
        let expected = vec![vec![
            Value::Null,
            Value::Int64(2),
            Value::Boolean(true),
            Value::Boolean(true),
        ]];
        assert_eq!(actual, expected);
    }
    assert!(resolver.hits.get() > 0);
}

#[test]
fn raw_and_error_aggregate_cells_remain_observable_inside_each_group() {
    let source = Data::new(&[(2, 1, "first"), (1, 2, "second")]);
    let sql = "SELECT G,Count(ChrW(55296)),Count(CVErr(Id)),\
               LenB(Min(ChrW(55296))),LenB(Max(ChrB(65))),CLng(Min(CVErr(Id))) \
               FROM T GROUP BY G";
    for declared in [false, true] {
        let actual = stream(&source, sql, options(declared)).unwrap();
        let expected = vec![
            vec![
                Value::Int32(2),
                Value::Int64(1),
                Value::Int64(1),
                Value::Int32(2),
                Value::Int32(1),
                Value::Int32(1),
            ],
            vec![
                Value::Int32(1),
                Value::Int64(1),
                Value::Int64(1),
                Value::Int32(2),
                Value::Int32(1),
                Value::Int32(2),
            ],
        ];
        assert_eq!(actual, expected);
    }
    let bare_raw = "SELECT G,Min(ChrW(55296)) FROM T GROUP BY G";
    assert!(matches!(
        stream(&source, bare_raw, options(false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::UnrepresentableText
        )))
    ));
}

#[test]
fn grouped_extrema_keep_unknown_declarations_and_active_union_carriers() {
    let source = Data::new(&[(2, 3, "first"), (1, 1, "second"), (2, 2, "third")]);
    let sql = "SELECT G,Min(Id) AS N FROM T GROUP BY G \
               UNION ALL SELECT G,CDbl(Max(Id)) FROM T GROUP BY G";
    let expected = vec![
        vec![Value::Int32(2), Value::Double(Double::from_float(2.0))],
        vec![Value::Int32(1), Value::Double(Double::from_float(1.0))],
        vec![Value::Int32(2), Value::Double(Double::from_float(3.0))],
        vec![Value::Int32(1), Value::Double(Double::from_float(1.0))],
    ];
    for declared in [false, true] {
        let actual = stream(&source, sql, options(declared)).unwrap();
        assert_eq!(actual, expected);
    }
    let unknown = "SELECT G,Min(Id)+'2' FROM T GROUP BY G";
    assert!(matches!(
        stream(&source, unknown, options(true)),
        Err(StreamError::Query(QueryError::ArithmeticDeclaration))
    ));
}
