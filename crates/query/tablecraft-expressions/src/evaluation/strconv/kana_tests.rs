use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expression)
}

#[test]
fn japanese_kana_flags_execute_in_a_real_scalar_query() {
    for (sql, expected) in [
        ("StrConv('ひらがな',16,1041)", "ヒラガナ"),
        ("StrConv('カタカナ',32,1041)", "かたかな"),
        ("StrConv('ゔゕゖゝゞ',16,1041)", "ヴヵヶヽヾ"),
        ("StrConv('ヴヵヶヽヾ',32,1041)", "ゔゕゖゝゞ"),
        (
            "StrConv('ヷヸヹヺ',32,1041)",
            "わ\u{3099}ゐ\u{3099}ゑ\u{3099}を\u{3099}",
        ),
        (
            "StrConv('か\u{3099} は\u{309a}',16,1041)",
            "カ\u{3099} ハ\u{309a}",
        ),
        ("StrConv('ﾊﾟ ｳﾞ カ゛ ー・Aß😀',32,1041)", "ﾊﾟ ｳﾞ か゛ ー・Aß😀"),
        ("StrConv(Mid('xひらがな',2),16,1041)", "ヒラガナ"),
        ("StrConv(LeftB('カタカナ',8),32,1041)", "かたかな"),
        (
            "StrConv(StrConv('こんにちは',16,1041),32,1041)",
            "こんにちは",
        ),
    ] {
        assert_eq!(evaluate(sql), Ok(Value::Text(expected.into())), "{sql}");
    }
    assert_eq!(evaluate("StrConv(NULL,16,1041)"), Ok(Value::Null));
    assert_eq!(
        evaluate("StrConv('',32,1041)"),
        Ok(Value::Text(String::new()))
    );
}

#[test]
fn kana_controls_keep_explicit_locale_raw_refusals_and_eager_failures() {
    for (sql, expected) in [
        ("StrConv('あ',16)", EvalError::UnsupportedExpression),
        ("StrConv(NULL,16,1033)", EvalError::UnsupportedExpression),
        ("StrConv('あ',16,66577)", EvalError::UnsupportedExpression),
        ("StrConv('あ',48,1041)", EvalError::Domain),
        ("StrConv('あ',17,1041)", EvalError::UnsupportedExpression),
        ("StrConv('あ',80,1041)", EvalError::UnsupportedExpression),
        ("StrConv('あ',4,1041)", EvalError::UnsupportedExpression),
        ("StrConv('あ',16,NULL)", EvalError::InvalidNullConversion),
        ("StrConv('あ',16,CVErr(7))", EvalError::TypeMismatch),
        (
            "StrConv(ChrW(55296),16,1041)",
            EvalError::UnrepresentableText,
        ),
        ("StrConv(ChrB(65),32,1041)", EvalError::UnrepresentableText),
        (
            "StrConv(ChrW(55296),16,1033)",
            EvalError::UnsupportedExpression,
        ),
        ("StrConv(ChrW(55296),16,1/0)", EvalError::DivisionByZero),
        ("StrConv('あ',16+0*(1/0),1041)", EvalError::DivisionByZero),
        ("StrConv('ゟ',16,1041)", EvalError::UnsupportedExpression),
        ("StrConv('ヿ',32,1041)", EvalError::UnsupportedExpression),
        ("StrConv('ㇰ',32,1041)", EvalError::UnsupportedExpression),
        ("StrConv('𛀁',16,1041)", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn kana_expansion_uses_shared_byte_work_text_and_cancellation_limits() {
    let expression = parse_expression("StrConv('ヷヸ',32,1041)", Default::default()).unwrap();
    let checks = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_cancellation(&|| {
            checks.set(checks.get() + 1);
            false
        })
        .evaluate(&expression)
        .unwrap();
    for stop in 1..=checks.get() {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| {
                    checks.set(checks.get() + 1);
                    checks.get() >= stop
                })
                .evaluate(&expression),
            Err(EvalError::Cancelled)
        );
    }
    for (bytes, steps) in [(0, 10000), (65536, 0)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expression),
            Err(EvalError::Limit(_))
        ));
    }
    // Input fits six UTF-8 bytes, but two voiced-wa expansions require twelve.
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 6,
                ..Default::default()
            }
        )
        .evaluate(&expression),
        Err(EvalError::TextLimit)
    );
}
