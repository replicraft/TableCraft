use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, ExpressionValue, Value},
};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::{conversions::text::NumberFormat, text::TextFunction},
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn direct(
    function: TextFunction,
    text: &str,
    format: Option<NumberFormat>,
) -> Result<Value, EvalError> {
    let value = ExpressionValue::StringBytes(ExpressionString::from_owned(
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ));
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    if let Some(format) = format {
        evaluator = evaluator.with_number_format(format);
    }
    if function == TextFunction::Str {
        evaluator.apply_expression_str(&value)
    } else {
        evaluator.apply_expression_radix(function, &value)
    }
}

#[test]
fn retained_kernel_and_bound_numeric_string_consumers_have_matching_values() {
    let source = MemorySource::default();
    for (text, format, expected) in [
        ("2.5", NumberFormat::EnUs, ["2", "2", " 2.5"]),
        ("3.5", NumberFormat::EnUs, ["4", "4", " 3.5"]),
        ("1.234,5", NumberFormat::DeDe, ["4D2", "2322", " 1234.5"]),
    ] {
        for ((name, function), expected) in [
            ("Hex", TextFunction::Hex),
            ("Oct", TextFunction::Oct),
            ("Str", TextFunction::Str),
        ]
        .into_iter()
        .zip(expected)
        {
            let kernel = direct(function, text, Some(format)).unwrap();
            assert_eq!(kernel, Value::Text(expected.into()));
            for input in [
                format!("Mid('{text}',1)"),
                format!("IIf(True,Mid('{text}',1),'unused')"),
                format!("CVar(Mid('{text}',1))"),
            ] {
                let sql = format!("SELECT {name}({input})");
                let parsed = parse_query(&sql, Default::default()).unwrap();
                let result = prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(
                        &source,
                        &[],
                        ExecutionOptions {
                            number_format: Some(format),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(result.rows()[0][0], kernel, "{sql}");
            }
        }
    }
}

#[test]
fn selected_unit_ranges_and_failure_boundaries_keep_existing_query_behavior() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Hex(Mid(ChrW(55296)&'12'&ChrW(55296),2,2)),Oct(Mid('x12y',2,2)),Str(Mid('x12y',2,2))",
        Default::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows()[0],
        [
            Value::Text("C".into()),
            Value::Text("14".into()),
            Value::Text(" 12".into())
        ]
    );

    for (name, function) in [
        ("Hex", TextFunction::Hex),
        ("Oct", TextFunction::Oct),
        ("Str", TextFunction::Str),
    ] {
        assert_eq!(
            direct(function, "12", None),
            Err(EvalError::NumberFormatRequired)
        );
        let parsed =
            parse_query(&format!("SELECT {name}(Mid('12',1))"), Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
        for (input, failure) in [
            ("ChrW(55296)", EvalError::UnrepresentableText),
            ("ChrB(49)", EvalError::UnrepresentableText),
            ("IIf(True,'12',1/0)", EvalError::DivisionByZero),
            ("Mid('bad',1)", EvalError::TypeMismatch),
        ] {
            let sql = format!("SELECT {name}({input})");
            let parsed = parse_query(&sql, Default::default()).unwrap();
            assert_eq!(
                prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(
                        &source,
                        &[],
                        ExecutionOptions {
                            number_format: Some(NumberFormat::EnUs),
                            ..Default::default()
                        },
                    )
                    .err(),
                Some(failure.into()),
                "{sql}"
            );
        }
    }
}
