use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expression)
}

#[test]
fn explicit_wide_hiragana_composition_widens_before_script_conversion() {
    for (input, expected) in [
        ("ｶﾞﾊﾟｳﾞ", "がぱゔ"),
        ("ｱｲｳｴｵ ｧｨｩｪｫ", "あいうえお　ぁぃぅぇぉ"),
        ("ABC123｡｢｣､･ｰ", "ＡＢＣ１２３。「」、・ー"),
        ("は゛ カ゛ ハ゜", "ば　が　ぱ"),
        ("ﾜﾞｦﾞ", "わ\u{3099}を\u{3099}"),
        ("ガガヵヶヽヾ", "ががゕゖゝゞ"),
        ("ひらがな①㍑ﬁe\u{301}😀", "ひらがな①㍑ﬁｅ\u{301}😀"),
        ("", ""),
    ] {
        let sql = format!("StrConv('{input}',36,1041)");
        assert_eq!(evaluate(&sql), Ok(Value::Text(expected.into())), "{sql}");
    }
    assert_eq!(evaluate("StrConv(NULL,36,1041)"), Ok(Value::Null));

    let raw = "StrConv(CVar(MidB(ChrW(55296)&'ｶﾞ'&ChrW(56320),3,4)),36,1041)";
    assert_eq!(evaluate(raw), Ok(Value::Text("が".into())));
    assert_eq!(
        evaluate("StrConv(StrConv('ｶﾞ',32,1041),4,1041)"),
        Ok(Value::Text("ガ".into()))
    );
}

#[test]
fn composition_keeps_legacy_recipes_and_unsupported_combinations_explicit() {
    assert_eq!(
        strconv::Conversion::from_code(36),
        Err(EvalError::UnsupportedExpression)
    );
    for (sql, expected) in [
        ("StrConv('ｶﾞ',36)", EvalError::UnsupportedExpression),
        ("StrConv(NULL,36,1033)", EvalError::UnsupportedExpression),
        ("StrConv('ｶﾞ',36,66577)", EvalError::UnsupportedExpression),
        ("StrConv('ｶﾞ',36,NULL)", EvalError::InvalidNullConversion),
        ("StrConv('ｶﾞ',36,CVErr(7))", EvalError::TypeMismatch),
        ("StrConv('ｶﾞ',100,1041)", EvalError::UnsupportedExpression),
        ("StrConv('ｶﾞ',164,1041)", EvalError::UnsupportedExpression),
        ("StrConv('ｶﾞ',44,1041)", EvalError::Domain),
        (
            "StrConv(ChrW(55296),36,1041)",
            EvalError::UnrepresentableText,
        ),
        ("StrConv(ChrB(65),36,1041)", EvalError::UnrepresentableText),
        (
            "StrConv(ChrW(55296),36,1033)",
            EvalError::UnsupportedExpression,
        ),
        ("StrConv(ChrW(55296),36,1/0)", EvalError::DivisionByZero),
        (
            "StrConv(IIf(True,'ｶﾞ',1/0),36,1041)",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn both_transformation_stages_share_work_allocations_and_every_cancel_checkpoint() {
    let expression = parse_expression("StrConv('Aｶﾞﾊﾟ',36,1041)", Default::default()).unwrap();
    let checks = Cell::new(0);
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    let observe = || {
        checks.set(checks.get() + 1);
        false
    };
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_cancellation(&observe)
        .evaluate(&expression)
        .unwrap();
    assert!(checks.get() > 0);

    for stop in 1..=checks.get() {
        let current = Cell::new(0);
        let cancelled = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&cancelled)
                .evaluate(&expression),
            Err(EvalError::Cancelled)
        );
    }
    for (bytes, steps) in [(0, 10_000), (65_536, 0)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expression),
            Err(EvalError::Limit(_))
        ));
    }

    let expansion = parse_expression("StrConv('A',36,1041)", Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    let limits = EvalLimits {
        max_text_bytes: 2,
        ..Default::default()
    };
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, limits).evaluate(&expansion),
        Err(EvalError::TextLimit)
    );
}
