use super::{EvalLimits, Evaluator, NoBindings};
use crate::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        dates::{FirstDayOfWeek, FirstWeekOfYear, parsing::DateTextFormat},
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

#[test]
fn format_date_masks_route_week_settings_and_preserve_numeric_date_coercion() {
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_date_format(DateTextFormat::Iso);
    for (sql, expected) in [
        (
            "Format(#1993-01-27 17:04:23#,'yyyy-mm-dd hh:nn:ss')",
            "1993-01-27 17:04:23",
        ),
        (
            "Format(#1993-01-27 17:04:23#,'hh:mm:ss am/pm')",
            "05:04:23 pm",
        ),
        ("Format(0,'yyyy-mm-dd')", "1899-12-30"),
        ("Format('1993-01-27','yyyy/mm/dd')", "1993-01-27"),
        ("Format(#2018-12-31#,'w ww',2,2)", "1 53"),
        ("Format(Null,'yyyy')", ""),
    ] {
        assert_eq!(
            evaluator.evaluate(&expression(sql)),
            Ok(Value::Text(expected.into())),
            "{sql}"
        );
    }
    assert_eq!(
        evaluate("Format(#1993-01-27#,'yyyy-mm-dd')"),
        Ok(Value::Text("1993-01-27".into()))
    );
    assert_eq!(
        evaluate("Format(#1993-01-27#,'dddd')"),
        Err(EvalError::DateFormatRequired)
    );
}

#[test]
fn format_date_lexing_conversion_and_output_check_every_cancellation_point() {
    let expr = expression("Format('1993-01-27 17:04:23','dddd, mmmm d yyyy hh:mm:ss AM/PM',2,2)");
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 10_000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_date_format(DateTextFormat::EnUs)
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
                .with_date_format(DateTextFormat::EnUs)
                .with_cancellation(&cancel)
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
}

#[test]
fn text_format_sections_and_unicode_use_the_shared_casing_policy() {
    let locale = "und".parse().unwrap();
    for (sql, expected) in [
        ("Format('Ab','@@@@')", "  Ab"),
        ("Format('Ab','!@@@@')", "Ab  "),
        ("Format('Ab','@&@&')", " Ab"),
        ("Format(Null,'@@;\"missing\"')", "missing"),
        ("Format('','@@;\"missing\"')", "missing"),
        ("Format('x','@@;\"missing\"')", " x"),
        ("Format('','@@;')", ""),
        ("Format('🦀','@@')", "🦀"),
        ("Format('straße','>\"label:\"@@@@@@')", "label:STRASSE"),
    ] {
        let mut budget = ResourceBudget::new(1000, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_case_locale(&locale)
                .evaluate(&expression(sql)),
            Ok(Value::Text(expected.into())),
            "{sql}"
        );
    }
    for (sql, error) in [
        ("Format('🦀','@-@')", EvalError::UnrepresentableText),
        ("Format('x','@@;@@;@@')", EvalError::UnsupportedExpression),
        ("Format(Null,'@@',1/0)", EvalError::DivisionByZero),
        ("Format('x','>@@')", EvalError::CaseLocaleRequired),
    ] {
        assert_eq!(evaluate(sql), Err(error), "{sql}");
    }
}

#[test]
fn text_format_casing_and_layout_preserve_every_cancellation_checkpoint() {
    let locale = "und".parse().unwrap();
    let expr = expression("Format('straße🦀','>\"label:\"@@@@@@@@@@')");
    let checkpoints = Cell::new(0);
    let mut budget = ResourceBudget::new(1000, 0, 10_000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_case_locale(&locale)
        .with_cancellation(&|| {
            checkpoints.set(checkpoints.get() + 1);
            false
        })
        .evaluate(&expr)
        .unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        let mut budget = ResourceBudget::new(1000, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_case_locale(&locale)
                .with_cancellation(&|| {
                    checkpoints.set(checkpoints.get() + 1);
                    checkpoints.get() == stop
                })
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
    for (bytes, steps, text_limit, resource) in [
        (1, 10_000, 100, Some(Resource::Bytes)),
        (1000, 10, 100, Some(Resource::Steps)),
        (1000, 10_000, 14, None),
    ] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        let result = Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: text_limit,
                ..Default::default()
            },
        )
        .with_case_locale(&locale)
        .evaluate(&expr);
        match resource {
            Some(expected) => assert!(
                matches!(result, Err(EvalError::Limit(error)) if error.resource == expected)
            ),
            None => assert_eq!(result, Err(EvalError::TextLimit)),
        }
    }
}
