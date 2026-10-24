use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, ExpressionValue, LegacyDateTime, Value},
};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::{
        conversions::text::NumberFormat,
        dates::{DateFunction, parsing::DateTextFormat},
    },
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{parse_query, parse_select, select::Projection};

fn cases() -> Vec<(&'static str, Vec<&'static str>, Value)> {
    let date = "1900-01-01 12:34:56";
    vec![
        ("Year", vec![date], Value::Int16(1900)),
        ("Month", vec![date], Value::Int16(1)),
        ("Day", vec![date], Value::Int16(1)),
        ("Hour", vec![date], Value::Int16(12)),
        ("Minute", vec![date], Value::Int16(34)),
        ("Second", vec![date], Value::Int16(56)),
        ("Weekday", vec![date, "2"], Value::Int16(1)),
        (
            "DateAdd",
            vec!["d", "1.5", "1900-01-01 12:00:00"],
            Value::DateTime(LegacyDateTime::from_ole_days(4.5).unwrap()),
        ),
        ("DatePart", vec!["q", date, "1", "1"], Value::Int16(1)),
        (
            "DateDiff",
            vec!["d", date, "1900-01-03 12:34:56", "1", "1"],
            Value::Int32(2),
        ),
    ]
}

fn retained(input: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(input.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn apply(
    evaluator: &mut Evaluator<'_>,
    name: &str,
    arguments: &[ExpressionValue],
) -> Result<Value, EvalError> {
    match name {
        "Weekday" => evaluator.apply_expression_weekday(&arguments[0], arguments.get(1)),
        "DateAdd" => {
            evaluator.apply_expression_date_add(&arguments[0], &arguments[1], &arguments[2])
        }
        "DatePart" => evaluator.apply_expression_date_part(
            &arguments[0],
            &arguments[1],
            arguments.get(2),
            arguments.get(3),
        ),
        "DateDiff" => evaluator.apply_expression_date_diff(
            &arguments[0],
            &arguments[1],
            &arguments[2],
            arguments.get(3),
            arguments.get(4),
        ),
        _ => evaluator
            .apply_expression_date_function(DateFunction::resolve(name).unwrap(), &arguments[0]),
    }
}

#[test]
fn all_ten_date_calls_agree_across_retained_materialized_scalar_and_bound_paths() {
    let source = MemorySource::default();
    for (name, arguments, expected) in cases() {
        for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
            let arguments: Vec<_> = arguments
                .iter()
                .copied()
                .map(|input| {
                    if format == NumberFormat::DeDe && name == "DateAdd" && input == "1.5" {
                        "1,5".to_owned()
                    } else {
                        input.to_owned()
                    }
                })
                .collect();
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(format)
                .with_date_format(DateTextFormat::Iso);
            for retained_input in [false, true] {
                let values: Vec<_> = arguments
                    .iter()
                    .map(|input| {
                        if retained_input {
                            retained(input)
                        } else {
                            ExpressionValue::Value(Value::Text(input.clone()))
                        }
                    })
                    .collect();
                assert_eq!(
                    apply(&mut evaluator, name, &values),
                    Ok(expected.clone()),
                    "{name} retained={retained_input}"
                );
            }
            let arguments = arguments
                .iter()
                .map(|input| format!("CVar(IIf(True,Mid('{input}',1),'unused'))"))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!("SELECT {name}({arguments})");
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
fn bound_date_calls_preserve_raw_refusals_and_eager_expression_errors() {
    let source = MemorySource::default();
    for (name, arguments, _) in cases() {
        let date_position = match name {
            "DateAdd" => 2,
            "DatePart" | "DateDiff" => 1,
            _ => 0,
        };
        for (input, expected) in [
            ("ChrW(55296)", EvalError::UnrepresentableText),
            ("ChrB(49)", EvalError::UnrepresentableText),
            ("IIf(True,'1900-01-01',1/0)", EvalError::DivisionByZero),
        ] {
            let mut arguments: Vec<_> = arguments
                .iter()
                .map(|argument| format!("'{argument}'"))
                .collect();
            arguments[date_position] = input.to_owned();
            let query = parse_query(
                &format!("SELECT {name}({})", arguments.join(",")),
                Default::default(),
            )
            .unwrap();
            let error = prepare(query.query(), &source)
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
                .err();
            assert_eq!(error, Some(expected.into()), "{name} {input}");
        }
    }
}
