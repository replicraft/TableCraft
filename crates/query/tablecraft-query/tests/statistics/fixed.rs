//! Fixed-point translation precedes the floating statistical kernel.
use super::*;
use tablecraft_core::values::{Currency, Decimal};

fn check(values: Vec<Value>, kind: ScalarType, sample: f64, population: f64) {
    let data = source(kind, values);
    let result = run("SELECT Var(X),VarP(X),StDev(X),StDevP(X) FROM T", &data).unwrap();
    for (actual, expected) in
        result.rows()[0]
            .iter()
            .zip([sample, population, sample.sqrt(), population.sqrt()])
    {
        near(float(actual), expected);
    }
}

#[test]
fn adjacent_large_longlong_currency_and_decimal_values_keep_their_spread() {
    for base in [9007199254740992, i64::MAX - 2, i64::MIN] {
        check(
            [base, base + 1, base + 2]
                .into_iter()
                .map(Value::Int64)
                .collect(),
            ScalarType::Int64,
            1.0,
            2.0 / 3.0,
        );
    }
    check(
        [i64::MAX - 2, i64::MAX - 1, i64::MAX]
            .into_iter()
            .map(|value| Value::Currency(Currency::from_scaled(value)))
            .collect(),
        ScalarType::Currency,
        1e-8,
        2e-8 / 3.0,
    );
    // Independent 96-bit decimal boundary and scale, rather than engine constants.
    let maximum = 79228162514264337593543950335_i128;
    for sign in [1, -1] {
        check(
            [maximum - 2, maximum - 1, maximum]
                .into_iter()
                .map(|value| Value::Decimal(Decimal::new(sign * value, 28).unwrap()))
                .collect(),
            ScalarType::Decimal,
            1e-56,
            2e-56 / 3.0,
        );
    }
}

#[test]
fn unequal_decimal_scales_and_wide_aligned_coefficients_remain_numeric() {
    check(
        vec![
            Value::Decimal(Decimal::new(1, 0).unwrap()),
            Value::Decimal(Decimal::new(10000000000000000000000000001, 28).unwrap()),
            Value::Decimal(Decimal::new(10000000000000000000000000002, 28).unwrap()),
        ],
        ScalarType::Decimal,
        1e-56,
        2e-56 / 3.0,
    );
    let maximum = 79228162514264337593543950335_i128;
    check(
        vec![
            Value::Decimal(Decimal::new(maximum, 0).unwrap()),
            Value::Decimal(Decimal::new(1, 28).unwrap()),
            Value::Decimal(Decimal::new(-maximum, 0).unwrap()),
        ],
        ScalarType::Decimal,
        (maximum as f64).powi(2),
        (maximum as f64).powi(2) * 2.0 / 3.0,
    );
}

#[test]
fn mixed_floating_groups_retain_the_original_double_conversion_lane() {
    let data = source(
        ScalarType::Int64,
        vec![
            Value::Int64(9007199254740992),
            Value::Int64(9007199254740993),
            Value::Int64(9007199254740994),
        ],
    );
    for expression in ["IIf(G=0,X,CDbl(X))", "IIf(G=0,CDbl(X),X)"] {
        let result = run(
            &format!("SELECT Var({expression}),VarP({expression}) FROM T"),
            &data,
        )
        .unwrap();
        near(float(&result.rows()[0][0]), 4.0 / 3.0);
        near(float(&result.rows()[0][1]), 8.0 / 9.0);
    }
}

#[test]
fn exact_offsets_reach_grouped_derived_domain_and_crosstab_consumers() {
    let data = source(
        ScalarType::Int64,
        vec![
            Value::Int64(9007199254740992),
            Value::Int64(9007199254740993),
            Value::Int64(9007199254740994),
        ],
    );
    for sql in [
        "SELECT Var(D.X) FROM (SELECT X FROM T) AS D",
        "SELECT DVar('X','T')",
        "SELECT Var(X) FROM T GROUP BY 1+1",
        "TRANSFORM Var(X) SELECT 1 AS K FROM T GROUP BY 1 PIVOT G IN (0,1)",
    ] {
        let result = run(sql, &data).unwrap();
        if sql.starts_with("TRANSFORM") {
            near(float(&result.rows()[0][1]), 2.0);
            assert_eq!(result.rows()[0][2], Value::Null);
        } else {
            near(float(&result.rows()[0][0]), 1.0);
        }
    }
}

#[test]
fn fixed_offset_storage_is_bounded_and_charged_in_addition_to_the_original_lane() {
    use tablecraft_query::QueryLimits;

    let data = source(ScalarType::Int64, (0..20).map(Value::Int64).collect());
    let minimum = |sql: &str| {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &data).unwrap();
        let mut low = 0;
        let mut high = QueryLimits::MAX_BYTES;
        while low < high {
            let middle = (low + high) / 2;
            if plan
                .execute(
                    &data,
                    &[],
                    ExecutionOptions {
                        limits: QueryLimits {
                            bytes: middle,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .is_ok()
            {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    };
    let fixed = minimum("SELECT Var(X) FROM T");
    let floating = minimum("SELECT Var(CDbl(X)) FROM T");
    // Twenty extra retained binary64 offsets, independent of production constants.
    assert_eq!(fixed - floating, 160);
}
