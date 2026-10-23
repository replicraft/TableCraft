use tablecraft_core::values::{Currency, Double, Single, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn run(source: &MemorySource, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    let result = prepare(parsed.query(), source)?.execute(source, &[], Default::default())?;
    Ok(result.rows().to_vec())
}

#[test]
fn conditional_currency_and_floating_inputs_work_in_both_orders() {
    let source = MemorySource::default();
    for conversion in ["CSng", "CDbl"] {
        for condition in ["N=1", "N=2"] {
            let expression = format!("IIf({condition},CCur(1.25),{conversion}(1.5))");
            let sql =
                format!("SELECT Sum({expression}) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q");
            assert_eq!(
                run(&source, &sql).unwrap(),
                vec![vec![Value::Double(Double::from_float(2.75))]]
            );
        }
    }
}

#[test]
fn typed_fields_and_domain_sums_convert_currency_extrema_with_decimal_rounding() {
    // Independent binary64 goldens for scaled i64 extrema / 10,000. Casting the
    // integer before division is not the decimal-to-binary rounding contract.
    for (scaled, bits) in [
        (i64::MIN, 0xc30a_36e2_eb1c_432d),
        (i64::MAX, 0x430a_36e2_eb1c_432d),
        (1, 0x3f1a_36e2_eb1c_432d),
        (-1, 0xbf1a_36e2_eb1c_432d),
    ] {
        for floating_type in [ScalarType::Single, ScalarType::Double] {
            for reverse in [false, true] {
                let zero = if floating_type == ScalarType::Single {
                    Value::Single(Single::from_float(0.0))
                } else {
                    Value::Double(Double::from_float(0.0))
                };
                let mut rows = vec![
                    vec![
                        Value::Int32(1),
                        Value::Currency(Currency::from_scaled(scaled)),
                        zero.clone(),
                    ],
                    vec![Value::Int32(2), Value::Null, zero],
                    vec![Value::Int32(3), Value::Null, Value::Null],
                ];
                if reverse {
                    rows.reverse();
                }
                let source = MemorySource::new(vec![
                    MemoryTable::new(
                        "T".into(),
                        vec![
                            Column::new("N".into(), ScalarType::Int32, false).unwrap(),
                            Column::new("Money".into(), ScalarType::Currency, true).unwrap(),
                            Column::new("Floating".into(), floating_type, true).unwrap(),
                        ],
                        rows,
                    )
                    .unwrap(),
                ])
                .unwrap();
                let sql = "SELECT Sum(IIf(N=1,[Money],Floating)),DSum('IIf(N=1,[Money],Floating)','T') FROM T";
                let expected = Value::Double(Double::from_bits(bits));
                assert_eq!(
                    run(&source, sql).unwrap(),
                    vec![vec![expected.clone(), expected]]
                );
            }
        }
    }
}

#[test]
fn fixed_currency_prefixes_keep_their_carrier_and_checked_overflow() {
    let source = MemorySource::default();
    let result = run(
        &source,
        "SELECT Sum(IIf(N<3,CCur(1.25),CDbl(0))),Sum(NULL) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q",
    )
    .unwrap();
    assert_eq!(
        result,
        vec![vec![
            Value::Currency(Currency::from_scaled(25000)),
            Value::Null
        ]]
    );

    // A later float does not retroactively repair a fixed-prefix overflow.
    let sql = "SELECT Sum(IIf(N<3,CCur(922337203685477),CDbl(0))) FROM (SELECT 1 AS N UNION ALL SELECT 2 UNION ALL SELECT 3) AS Q";
    assert_eq!(run(&source, sql), Err(EvalError::Overflow.into()));
}
