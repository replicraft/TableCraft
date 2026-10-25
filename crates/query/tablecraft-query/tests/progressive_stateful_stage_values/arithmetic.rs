use super::*;

#[test]
fn inner_global_all_five_kernels_keep_integer_decimal_and_currency_carriers() {
    for (kind, values, expected) in [
        (
            ScalarType::Int32,
            vec![Value::Int32(1), Value::Null, Value::Int32(3)],
            vec![
                Value::Int64(4),
                Value::Double(Double::from_float(2.0)),
                Value::Int32(1),
                Value::Int32(3),
                Value::Int64(2),
            ],
        ),
        (
            ScalarType::Decimal,
            vec![
                Value::Decimal(Decimal::new(125, 2).unwrap()),
                Value::Null,
                Value::Decimal(Decimal::new(250, 2).unwrap()),
            ],
            vec![
                Value::Decimal(Decimal::new(375, 2).unwrap()),
                Value::Decimal(Decimal::new(18750, 4).unwrap()),
                Value::Decimal(Decimal::new(125, 2).unwrap()),
                Value::Decimal(Decimal::new(250, 2).unwrap()),
                Value::Int64(2),
            ],
        ),
        (
            ScalarType::Currency,
            vec![
                Value::Currency(Currency::from_scaled(12500)),
                Value::Null,
                Value::Currency(Currency::from_scaled(25000)),
            ],
            vec![
                Value::Currency(Currency::from_scaled(37500)),
                Value::Double(Double::from_float(1.875)),
                Value::Currency(Currency::from_scaled(12500)),
                Value::Currency(Currency::from_scaled(25000)),
                Value::Int64(2),
            ],
        ),
    ] {
        let source = Input::new(
            &[("V", kind)],
            values.into_iter().map(|value| vec![value]).collect(),
        );
        for prepare_declared in [false, true] {
            for declared_integral in [false, true] {
                let (summary, actual) = run(
                    "SELECT S,A,Lo,Hi,N FROM (SELECT Sum(V) AS S,Avg(V) AS A,Min(V) AS Lo,Max(V) AS Hi,Count(V) AS N FROM T) Q",
                    &source, &[], prepare_declared, declared_integral,
                ).unwrap();
                assert_eq!(
                    actual.as_slice(),
                    std::slice::from_ref(&expected),
                    "{kind:?}"
                );
                assert_eq!(summary.scanned_rows, 3);
                assert_eq!(summary.emitted_rows, 1);
            }
        }
    }
}

#[test]
fn empty_global_and_empty_grouped_stages_send_different_logical_populations() {
    let source = Input::new(&[("V", ScalarType::Int32)], Vec::new());
    for (sql, expected) in [
        (
            "SELECT Count(*) FROM (SELECT Count(*) AS N FROM T) Q",
            vec![vec![Value::Int64(1)]],
        ),
        (
            "SELECT Count(*) FROM (SELECT V FROM T GROUP BY V) Q",
            vec![vec![Value::Int64(0)]],
        ),
        (
            "SELECT Count(*) FROM (SELECT Count(*) AS N FROM T) Q WHERE N>0",
            vec![vec![Value::Int64(0)]],
        ),
        (
            "SELECT S,A,Lo,Hi,N FROM (SELECT Sum(V) AS S,Avg(V) AS A,Min(V) AS Lo,Max(V) AS Hi,Count(V) AS N FROM T) Q",
            vec![vec![
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Int64(0),
            ]],
        ),
    ] {
        let (summary, actual) = run(sql, &source, &[], false, false).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(summary.scanned_rows, 0);
        assert_eq!(summary.emitted_rows, 1);
    }
}

#[test]
fn grouped_inner_then_outer_global_uses_independently_modeled_totals() {
    let source = Input::fixture();
    let (_, actual) = run(
        "SELECT Sum(N),Count(*),Sum(Total) FROM (SELECT G,Count(*) AS N,Sum(Score) AS Total FROM T GROUP BY G) Q",
        &source, &[], false, false,
    ).unwrap();
    // Three original groups; counts2+2+2, scores1+2+4+5+6 (the third is NULL).
    assert_eq!(
        actual,
        [vec![Value::Int64(6), Value::Int64(3), Value::Int64(18)]]
    );
}

#[test]
fn outer_grouping_consumes_inner_group_results_without_reusing_input_row_layout() {
    let source = Input::fixture();
    let (_, actual) = run(
        "SELECT N,Count(*),Sum(N) FROM (SELECT G,Count(*) AS N FROM T GROUP BY G) Q GROUP BY N",
        &source,
        &[],
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        actual,
        [vec![Value::Int64(2), Value::Int64(3), Value::Int64(6)]]
    );
}
