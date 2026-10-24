use tablecraft_core::values::{Double, Single, Value};
use tablecraft_expressions::{
    EvalError,
    numeric::{self, Arithmetic},
};
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
fn floating_selected_sum_and_domain_sum_round_large_integers_in_both_orders() {
    // Binary64 nearest-even goldens for the integer itself, before summation.
    for (integer, bits) in [
        (9007199254740991, 0x433f_ffff_ffff_ffff),
        (9007199254740993, 0x4340_0000_0000_0000),
        (9007199254740995, 0x4340_0000_0000_0002),
        (-9007199254740993, 0xc340_0000_0000_0000),
        (i64::MAX, 0x43e0_0000_0000_0000),
        (i64::MIN, 0xc3e0_0000_0000_0000),
    ] {
        for floating_type in [ScalarType::Single, ScalarType::Double] {
            for reverse in [false, true] {
                let zero = if floating_type == ScalarType::Single {
                    Value::Single(Single::from_float(0.0))
                } else {
                    Value::Double(Double::from_float(0.0))
                };
                let mut rows = vec![
                    vec![Value::Int32(1), Value::Int64(integer), zero.clone()],
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
                            Column::new("Large".into(), ScalarType::Int64, true).unwrap(),
                            Column::new("Floating".into(), floating_type, true).unwrap(),
                        ],
                        rows,
                    )
                    .unwrap(),
                ])
                .unwrap();
                let expected = Value::Double(Double::from_bits(bits));
                assert_eq!(
                    run(&source, "SELECT Sum(IIf(N=1,Large,Floating)),DSum('IIf(N=1,Large,Floating)','T') FROM T").unwrap(),
                    vec![vec![expected.clone(), expected]],
                );
            }
        }
    }
}

#[test]
fn integer_prefix_overflow_and_encounter_dependent_rounding_stay_explicit() {
    let source = MemorySource::default();
    let input = "(SELECT 1 AS N UNION ALL SELECT 2 UNION ALL SELECT 3) AS Q";
    for (expression, bits) in [
        (
            "IIf(N=1,CDbl(0),IIf(N=2,CLngLng(9007199254740993),CLngLng(1)))",
            0x4340_0000_0000_0000,
        ),
        (
            "IIf(N=3,CDbl(0),IIf(N=1,CLngLng(9007199254740993),CLngLng(1)))",
            0x4340_0000_0000_0001,
        ),
    ] {
        assert_eq!(
            run(&source, &format!("SELECT Sum({expression}) FROM {input}")).unwrap(),
            vec![vec![Value::Double(Double::from_bits(bits))]],
        );
    }

    for expression in [
        "IIf(N=3,CDbl(0),CLngLng(9223372036854775807))",
        "IIf(N=3,CDbl(0),CLngLng(-9223372036854775807))",
    ] {
        assert_eq!(
            run(&source, &format!("SELECT Sum({expression}) FROM {input}")),
            Err(EvalError::Overflow.into()),
        );
    }

    // This aggregate conversion does not relax the strict arithmetic service.
    assert_eq!(
        numeric::arithmetic(
            Arithmetic::Add,
            &Value::Double(Double::from_float(0.0)),
            &Value::Int64(9007199254740993)
        ),
        Err(EvalError::PrecisionLoss),
    );
}

#[test]
fn floating_accumulation_does_not_admit_date_text_or_hide_late_errors() {
    let source = MemorySource::default();
    for (operand, error) in [
        ("#2024-01-01#", EvalError::TypeMismatch),
        ("'12'", EvalError::TypeMismatch),
        ("1/0", EvalError::DivisionByZero),
        ("CVErr(5)", EvalError::TypeMismatch),
    ] {
        assert_eq!(
            run(
                &source,
                &format!(
                    "SELECT Sum(IIf(N=1,CDbl(0),{operand})) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Q"
                )
            ),
            Err(error.into()),
        );
    }
}
