use super::*;
use tablecraft_core::values::Decimal;

impl Restricted {
    fn decimals(ignore: bool) -> Self {
        let rows = [
            None,
            Some((-1, 4)),
            Some((0, 4)),
            Some((1, 0)),
            Some((10000, 4)),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            vec![
                value.map_or(Value::Null, |(coefficient, scale)| {
                    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
                }),
                Value::Int32(index as i32 + 1),
            ]
        })
        .collect();

        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::Decimal), ("Y", ScalarType::Int32)],
                rows,
            )])
            .unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore,
        }
    }
}

fn parameter_query(
    source: &Restricted,
    filter: &str,
    value: Decimal,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let sql = format!(
        "PARAMETERS P Decimal(28,{}); SELECT Y FROM T WHERE {filter}",
        value.scale(),
    );
    let query = tablecraft_sql::parse_query(&sql, Default::default()).unwrap();
    prepare(query.query(), source)?
        .execute(source, &[("P", Value::Decimal(value))], Default::default())
        .map(|result| result.rows().to_vec())
}

#[test]
fn actual_decimal_parameter_points_retain_numeric_equality_across_scales_and_nulls() {
    for (coefficient, scale, ids) in [
        (1, 0, vec![4, 5]),
        (100, 2, vec![4, 5]),
        (10000, 4, vec![4, 5]),
        (-1, 4, vec![2]),
        (0, 28, vec![3]),
    ] {
        let value = Decimal::new(coefficient, scale).unwrap();
        for filter in ["X=P", "P=X", "X=P And X=P"] {
            let source = Restricted::decimals(false);
            let ignoring = Restricted::decimals(true);
            let actual = parameter_query(&source, filter, value).unwrap();
            assert_eq!(actual, parameter_query(&ignoring, filter, value).unwrap());
            assert_eq!(
                actual,
                ids.iter()
                    .map(|id| vec![Value::Int32(*id)])
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                source.seen.get(),
                Some(ScanConstraint::Decimal { column: 0, value })
            );
            assert_eq!(source.scans.get(), 0);
        }
    }
}

#[test]
fn decimal_null_hints_remain_distinct_and_not_null_or_conversion_forms_decline() {
    let source = Restricted::decimals(false);
    assert_eq!(
        run("SELECT Y FROM T WHERE X IS NULL", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(source.seen.get(), Some(ScanConstraint::Null { column: 0 }));

    let source = Restricted::decimals(false);
    let query = tablecraft_sql::parse_query(
        "PARAMETERS P Decimal(28,4); SELECT Y FROM T WHERE X=P",
        Default::default(),
    )
    .unwrap();
    assert!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[("P", Value::Null)], Default::default())
            .unwrap()
            .rows()
            .is_empty()
    );
    assert_eq!(source.seen.get(), None);

    for filter in [
        "X IS NOT NULL",
        "X=CDec(1)",
        "X=CDec('bad')",
        "X=CDec(1/0)",
        "X=1",
        "X='1'",
        "X>0",
    ] {
        let source = Restricted::decimals(false);
        let ignoring = Restricted::decimals(true);
        let sql = format!("SELECT Y FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).map(|result| result.rows().to_vec()),
            run(&sql, &ignoring).map(|result| result.rows().to_vec()),
        );
        assert_eq!(source.seen.get(), None);
    }
}

#[test]
fn decimal_ranges_partial_predicates_and_eager_failures_never_become_point_hints() {
    for filter in ["X<P", "X=P Or X IS NULL", "X=P And Y=4", "X=P And 1/0=0"] {
        let source = Restricted::decimals(false);
        let ignoring = Restricted::decimals(true);
        let value = Decimal::new(1, 0).unwrap();
        assert_eq!(
            parameter_query(&source, filter, value),
            parameter_query(&ignoring, filter, value)
        );
        assert_eq!(source.seen.get(), None);
    }
}
