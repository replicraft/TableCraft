use tablecraft_core::{
    limits::ResourceBudget,
    values::{
        Currency, Decimal, Double, ExpressionString, ExpressionValue, LegacyDateTime, Single, Value,
    },
};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::{
        conversions::{Conversion, text::NumberFormat},
        dates::parsing::{DateConversion, DateTextFormat},
    },
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{parse_query, parse_select, select::Projection};

fn cases() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("CByte", "2.5", Value::Byte(2)),
        ("CInt", "2.5", Value::Int16(2)),
        ("CLng", "2.5", Value::Int32(2)),
        ("CLngLng", "9223372036854775807", Value::Int64(i64::MAX)),
        ("CBool", "2.5", Value::Boolean(true)),
        ("CSng", "2.5", Value::Single(Single::from_float(2.5))),
        ("CDbl", "2.5", Value::Double(Double::from_float(2.5))),
        (
            "CCur",
            "922337203685477.5807",
            Value::Currency(Currency::from_scaled(i64::MAX)),
        ),
        (
            "CDec",
            "79228162514264337593543950335",
            Value::Decimal(Decimal::new(79228162514264337593543950335, 0).unwrap()),
        ),
        (
            "CDate",
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap()),
        ),
        (
            "CVDate",
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap()),
        ),
        (
            "DateValue",
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap()),
        ),
        (
            "TimeValue",
            "1900-01-01 12:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(0.5).unwrap()),
        ),
    ]
}

#[test]
fn all_thirteen_conversions_agree_across_retained_scalar_and_bound_consumers() {
    let source = MemorySource::default();
    for (name, input, expected) in cases() {
        for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
            let input = if format == NumberFormat::DeDe && Conversion::resolve(name).is_some() {
                input.replace('.', ",")
            } else {
                input.to_owned()
            };
            let raw = ExpressionValue::StringBytes(ExpressionString::from_owned(
                input.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            ));
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .with_date_format(DateTextFormat::Iso);
            let result = if let Some(target) = Conversion::resolve(name) {
                evaluator.apply_expression_conversion(target, &raw)
            } else {
                evaluator
                    .apply_expression_date_conversion(DateConversion::resolve(name).unwrap(), &raw)
            };
            assert_eq!(result, Ok(expected.clone()), "retained {name}");

            let sql = format!("SELECT {name}(CVar(IIf(True,Mid('{input}',1),'unused')))");
            let scalar = parse_select(&sql, Default::default()).unwrap();
            let Projection::Expression { expression, .. } = &scalar.query().projection[0] else {
                panic!("scalar projection required");
            };
            assert_eq!(
                evaluator.evaluate(expression),
                Ok(expected.clone()),
                "scalar {name}"
            );
            let query = parse_query(&sql, Default::default()).unwrap();
            let result = prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(format),
                        date_format: Some(DateTextFormat::Iso),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(result.rows()[0][0], expected, "bound {name}");
        }
    }
}

#[test]
fn bound_conversion_raw_failures_and_eager_errors_keep_their_original_meanings() {
    let source = MemorySource::default();
    for (name, _, _) in cases() {
        for (input, error) in [
            ("ChrW(55296)", EvalError::UnrepresentableText),
            ("ChrB(49)", EvalError::UnrepresentableText),
            ("IIf(True,'2',1/0)", EvalError::DivisionByZero),
        ] {
            let query =
                parse_query(&format!("SELECT {name}({input})"), Default::default()).unwrap();
            assert_eq!(
                prepare(query.query(), &source)
                    .unwrap()
                    .execute(
                        &source,
                        &[],
                        ExecutionOptions {
                            number_format: Some(NumberFormat::EnUs),
                            date_format: Some(DateTextFormat::Iso),
                            ..Default::default()
                        },
                    )
                    .err(),
                Some(error.into()),
                "{name}"
            );
        }
    }
}
