use super::{EvalLimits, Evaluator, NoBindings};
use crate::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        dates::{FirstDayOfWeek, FirstWeekOfYear},
        text::TextFunction,
    },
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::{
    parse_select,
    select::{Expression, Projection},
};

fn expression(text: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {text}"), Default::default()).unwrap();
    let Projection::Expression { expression, .. } = parsed.query().projection[0].clone() else {
        panic!("expected expression");
    };
    expression
}

fn evaluate(text: &str) -> Result<Value, EvalError> {
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .evaluate(&expression(text))
}

#[test]
fn format_defaults_optional_slots_named_styles_and_numeric_sections_execute() {
    for (sql, expected) in [
        ("Format(23)", "23"),
        ("Format(23,)", "23"),
        ("Format(23,,1,1)", "23"),
        ("Format(23,'',,1)", "23"),
        ("Format(1234,'#,##0.00')", "1,234.00"),
        ("Format(-12,'0;(0)')", "(12)"),
        ("Format(0,'0;(0);\"zero\"')", "zero"),
        ("Format(Null,'0;0;0;\"missing\"')", "missing"),
        ("Format(Null)", ""),
        ("Format(12,'Fixed')", "12.00"),
        ("Format(True,'Yes/No')", "Yes"),
        ("Format('1,234.5','0.00')", "1234.50"),
        ("Format(0.125,'0.00%')", "12.50%"),
    ] {
        assert_eq!(evaluate(sql), Ok(Value::Text(expected.into())), "{sql}");
    }
    let locale = "en-US".parse().unwrap();
    let mut budget = ResourceBudget::new(100, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_case_locale(&locale)
            .evaluate(&expression("Format('Hello','>')")),
        Ok(Value::Text("HELLO".into()))
    );
}

#[test]
fn format_keeps_eager_failures_and_distinguishes_omitted_settings_from_null() {
    for (sql, expected) in [
        ("Format(Null,'0',1/0)", EvalError::DivisionByZero),
        ("Format(23,Null)", EvalError::InvalidNullConversion),
        ("Format(23,'0',Null)", EvalError::InvalidNullConversion),
        ("Format(23,'0',8)", EvalError::Domain),
        ("Format(23,'0',1,4)", EvalError::Domain),
        ("Format(23,'0',0)", EvalError::CalendarPolicyRequired),
        ("Format(23,'0',1,0)", EvalError::CalendarPolicyRequired),
        ("Format(1,'0',1,1,1/0)", EvalError::UnsupportedExpression),
        ("Format(,1/0)", EvalError::UnsupportedExpression),
        ("Format(*)", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::EnUs)
            .with_first_day_of_week(FirstDayOfWeek::new(2).unwrap())
            .with_first_week_of_year(FirstWeekOfYear::new(2).unwrap())
            .evaluate(&expression("Format(23,'0',0,0)")),
        Ok(Value::Text("23".into()))
    );
}

#[test]
fn format_output_bytes_work_and_every_observed_cancellation_checkpoint_are_bounded() {
    let expr = expression("Format(-1234.567,'#,##0.00;(#,##0.00);\"zero\";\"missing\"')");
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 10_000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&count)
        .evaluate(&expr)
        .unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        let cancel = || {
            checkpoints.set(checkpoints.get() + 1);
            checkpoints.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancel)
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
    let arguments = [Value::Int32(123456), Value::Text("0".into())];
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 2,
                ..Default::default()
            }
        )
        .with_number_format(NumberFormat::EnUs)
        .apply_text_function(TextFunction::Format, &arguments),
        Err(EvalError::TextLimit)
    );
    let mut budget = ResourceBudget::new(0, 0, 1000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .apply_text_function(TextFunction::Format, &arguments);
    assert!(matches!(result, Err(EvalError::Limit(error)) if error.resource == Resource::Bytes));
}
