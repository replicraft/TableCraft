use super::*;

#[test]
fn scalar_families_preserve_independent_extremes_and_first_equal_carriers() {
    let cases = [
        (
            ScalarType::Boolean,
            vec![Value::Boolean(false), Value::Boolean(true)],
            Value::Boolean(true),
            Value::Boolean(false),
        ),
        (
            ScalarType::Byte,
            vec![Value::Byte(255), Value::Byte(0)],
            Value::Byte(0),
            Value::Byte(255),
        ),
        (
            ScalarType::Int16,
            vec![Value::Int16(i16::MAX), Value::Int16(i16::MIN)],
            Value::Int16(i16::MIN),
            Value::Int16(i16::MAX),
        ),
        (
            ScalarType::Int32,
            vec![Value::Int32(i32::MAX), Value::Int32(i32::MIN)],
            Value::Int32(i32::MIN),
            Value::Int32(i32::MAX),
        ),
        (
            ScalarType::Int64,
            vec![Value::Int64(i64::MAX), Value::Int64(i64::MIN)],
            Value::Int64(i64::MIN),
            Value::Int64(i64::MAX),
        ),
        (
            ScalarType::Single,
            vec![
                Value::Single(Single::from_float(3.0)),
                Value::Single(Single::from_float(-2.0)),
            ],
            Value::Single(Single::from_float(-2.0)),
            Value::Single(Single::from_float(3.0)),
        ),
        (
            ScalarType::Double,
            vec![
                Value::Double(Double::from_float(3.0)),
                Value::Double(Double::from_float(-2.0)),
            ],
            Value::Double(Double::from_float(-2.0)),
            Value::Double(Double::from_float(3.0)),
        ),
        (
            ScalarType::Currency,
            vec![
                Value::Currency(Currency::from_scaled(31)),
                Value::Currency(Currency::from_scaled(-21)),
            ],
            Value::Currency(Currency::from_scaled(-21)),
            Value::Currency(Currency::from_scaled(31)),
        ),
        (
            ScalarType::Decimal,
            vec![decimal(300, 2), decimal(-2000, 3)],
            decimal(-2000, 3),
            decimal(300, 2),
        ),
        (
            ScalarType::DateTime,
            vec![
                Value::DateTime(LegacyDateTime::from_ole_days(3.0).unwrap()),
                Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
            ],
            Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(3.0).unwrap()),
        ),
    ];
    for (kind, values, minimum, maximum) in cases {
        for declared in [false, true] {
            let mut values = values.clone();
            values.insert(1, Value::Null);
            let source = Data::new(kind, values);
            let sql = "SELECT Min(N),Max(N),Count(N) FROM T";
            let actual = stream(&source, sql, options(declared)).unwrap();
            assert_eq!(
                actual,
                [vec![minimum.clone(), maximum.clone(), Value::Int64(2)]],
                "{kind:?}"
            );

            let syntax = parse_query(sql, Default::default()).unwrap();
            let buffered = prepare(syntax.query(), &source)
                .unwrap()
                .execute(&source, &[], options(declared))
                .unwrap();
            assert_eq!(actual, buffered.rows(), "{kind:?}");
        }
    }
}

#[test]
fn empty_null_and_filtered_populations_publish_one_null_result_after_eof() {
    for values in [vec![], vec![Value::Null], vec![Value::Int32(2)]] {
        let source = Data::new(ScalarType::Int32, values);
        for declared in [false, true] {
            let actual = stream(
                &source,
                "SELECT Min(N),Max(N),Count(*) FROM T WHERE False",
                options(declared),
            )
            .unwrap();
            assert_eq!(actual, [vec![Value::Null, Value::Null, Value::Int64(0)]]);
        }
    }
}

#[test]
fn retained_raw_and_error_cells_are_observed_without_premature_publication() {
    let source = Data::new(ScalarType::Int32, vec![Value::Int32(1)]);
    for declared in [false, true] {
        let actual = stream(
            &source,
            "SELECT LenB(Min(ChrW(55296))),LenB(Max(ChrB(65))) FROM T",
            options(declared),
        )
        .unwrap();
        assert_eq!(actual, [vec![Value::Int32(2), Value::Int32(1)]]);
        assert!(stream(&source, "SELECT Min(ChrW(55296)) FROM T", options(declared)).is_err());
    }
    let source = Data::new(
        ScalarType::Int32,
        vec![Value::Int32(65535), Value::Int32(0), Value::Int32(7)],
    );
    let actual = stream(
        &source,
        "SELECT CLng(Min(CVErr(N))),CLng(Max(CVErr(N))) FROM T",
        options(false),
    )
    .unwrap();
    assert_eq!(actual, [vec![Value::Int32(0), Value::Int32(65535)]]);
    assert!(stream(&source, "SELECT Min(CVErr(N)) FROM T", options(false)).is_err());
}

#[test]
fn extrema_declarations_stay_unknown_and_union_uses_existing_active_promotions() {
    let source = Data::new(ScalarType::Int32, vec![Value::Int32(3), Value::Int32(1)]);
    let sql = "SELECT Min(N) AS Low,Max(N) AS High,Low+High FROM T";
    assert_eq!(
        stream(&source, sql, options(true)).unwrap(),
        [vec![Value::Int32(1), Value::Int32(3), Value::Int32(4)]]
    );
    assert!(matches!(
        stream(&source, "SELECT Min(N)+'2' FROM T", options(true)),
        Err(StreamError::Query(QueryError::ArithmeticDeclaration))
    ));
    let union = "SELECT Min(N) AS V FROM T UNION ALL SELECT CDbl(Max(N)) FROM T";
    assert_eq!(
        stream(&source, union, options(false)).unwrap(),
        [
            vec![Value::Double(Double::from_float(1.0))],
            vec![Value::Double(Double::from_float(3.0))]
        ]
    );
}
