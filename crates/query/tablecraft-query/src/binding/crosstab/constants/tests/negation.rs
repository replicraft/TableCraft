//! Closed-heading negation agrees with actual declared query execution.
use super::*;
use crate::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_core::values::{Double, LegacyDateTime};
use tablecraft_schema::ScalarType;
use tablecraft_sql::parse_query;

fn runtime_negation(value: Value, variant: bool) -> Result<Value, QueryError> {
    let kind = ScalarType::of(&value).unwrap();
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("V".into(), kind, false).unwrap()],
            vec![vec![value]],
        )
        .unwrap(),
    ])
    .unwrap();
    let sql = if variant {
        "SELECT -CVar(V) FROM T"
    } else {
        "SELECT -V FROM T"
    };
    let syntax = parse_query(sql, Default::default()).unwrap();
    let result = prepare(syntax.query(), &source)?.execute(
        &source,
        &[],
        ExecutionOptions {
            declared_integral: true,
            ..Default::default()
        },
    )?;
    Ok(result.rows()[0][0].clone())
}

fn heading_negation(value: Value, variant: bool) -> Result<Value, QueryError> {
    let mut binder = binder(None, None);
    binder.declared_arithmetic = true;
    let operand = match value {
        Value::DateTime(value) => Expr::DateLiteral("raw serial fixture".into(), value),
        value => Expr::NumericLiteral(value),
    };
    let operand = if variant {
        Expr::CVar(Box::new(operand))
    } else {
        operand
    };
    binder.constant_heading(&Expr::Unary(UnaryOperator::Minus, Box::new(operand)), 1)
}

#[test]
fn date_heading_negation_keeps_runtime_serial_bits_carriers_and_envelope() {
    for (input, output, overflows_date) in [
        (-657434.75, 657434.75, false),
        (-1.25, 1.25, false),
        (-0.25, 0.25, false),
        (-0.0, 0.0, false),
        (0.0, 0.0, false),
        (0.25, -0.25, false),
        (657434.75, -657434.75, false),
        (2958465.75, -2958465.75, true),
    ] {
        for variant in [false, true] {
            let value = Value::DateTime(LegacyDateTime::from_ole_days(input).unwrap());
            let expected = if overflows_date && !variant {
                Err(QueryError::Evaluation(EvalError::Overflow))
            } else if overflows_date {
                Ok(Value::Double(Double::from_float(output)))
            } else {
                Ok(Value::DateTime(
                    LegacyDateTime::from_ole_days(output).unwrap(),
                ))
            };
            let runtime = runtime_negation(value.clone(), variant);
            let heading = heading_negation(value, variant);
            assert_eq!(runtime, expected, "serial {input:?}, Variant {variant}");
            assert_eq!(heading, expected, "serial {input:?}, Variant {variant}");

            // Raw serial bits are checked explicitly: both signs of input zero
            // produce positive zero under the current 0 - DATE serial policy.
            if let Ok(Value::DateTime(value)) = heading {
                assert_eq!(value.ole_days().to_bits(), output.to_bits());
            }
        }
    }
}

#[test]
fn narrow_heading_negation_keeps_scalar_overflow_and_variant_widening() {
    for (input, widened) in [
        (Value::Int16(i16::MIN), Value::Int32(32768)),
        (
            Value::Int32(i32::MIN),
            Value::Double(Double::from_float(2147483648.0)),
        ),
    ] {
        for variant in [false, true] {
            let expected = if variant {
                Ok(widened.clone())
            } else {
                Err(QueryError::Evaluation(EvalError::Overflow))
            };
            assert_eq!(runtime_negation(input.clone(), variant), expected);
            assert_eq!(heading_negation(input.clone(), variant), expected);
        }
    }
}
