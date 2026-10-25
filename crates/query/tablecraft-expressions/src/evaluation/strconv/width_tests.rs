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
fn japanese_width_goldens_keep_script_and_unrelated_compatibility_characters() {
    for (sql, expected) in [
        ("StrConv('ABC 123 !~',4,1041)", "ＡＢＣ　１２３　！～"),
        ("StrConv('ＡＢＣ　１２３　！～',8,1041)", "ABC 123 !~"),
        ("StrConv('｡｢｣､･ｦｧｰｱｶﾊﾝ',4,1041)", "。「」、・ヲァーアカハン"),
        ("StrConv('。「」、・ヲァーアカハン',8,1041)", "｡｢｣､･ｦｧｰｱｶﾊﾝ"),
        ("StrConv('ｶﾞﾊﾟｳﾞﾜﾞｦﾞ',4,1041)", "ガパヴヷヺ"),
        ("StrConv('ガパヴヷヺ',8,1041)", "ｶﾞﾊﾟｳﾞﾜﾞｦﾞ"),
        ("StrConv('は゛がハ゜',4,1041)", "ばがパ"),
        ("StrConv('A゛゜',4,1041)", "Ａ゛゜"),
        ("StrConv('カ゛ハ゜',8,1041)", "ｶ゛ﾊ゜"),
        ("StrConv('⦅⦆',4,1041)", "｟｠"),
        ("StrConv('｟｠',8,1041)", "⦅⦆"),
        ("StrConv('゛゜',8,1041)", "゛゜"),
        ("StrConv('ﾞﾟ',4,1041)", "\u{3099}\u{309a}"),
        ("StrConv('ガパ',8,1041)", "ｶﾞﾊﾟ"),
        ("StrConv('ひらがな①㍑ﬁé😀',4,1041)", "ひらがな①㍑ﬁｅ́😀"),
        ("StrConv('ひらがな①㍑ﬁé😀',8,1041)", "ひらがな①㍑ﬁé😀"),
        ("StrConv(LeftB('ｶﾞ',4),4,1041)", "ガ"),
        ("StrConv(Mid('xガ',2),8,1041)", "ｶﾞ"),
        ("StrConv(Mid(ChrW(55296)&'ｶﾞ',2),4,1041)", "ガ"),
        ("StrConv('',4,1041)", ""),
    ] {
        assert_eq!(evaluate(sql), Ok(Value::Text(expected.into())), "{sql}");
    }
    assert_eq!(evaluate("StrConv(NULL,8,1041)"), Ok(Value::Null));
}

#[test]
fn width_controls_raw_data_and_unqualified_forms_fail_in_existing_order() {
    for (sql, expected) in [
        ("StrConv('A',4)", EvalError::UnsupportedExpression),
        ("StrConv(NULL,8,1033)", EvalError::UnsupportedExpression),
        ("StrConv('A',4,66577)", EvalError::UnsupportedExpression),
        ("StrConv('A',12,1041)", EvalError::Domain),
        ("StrConv('A',5,1041)", EvalError::UnsupportedExpression),
        ("StrConv('A',20,1041)", EvalError::UnsupportedExpression),
        ("StrConv('A',4,NULL)", EvalError::InvalidNullConversion),
        (
            "StrConv(ChrW(55296),4,1041)",
            EvalError::UnrepresentableText,
        ),
        ("StrConv(ChrB(65),8,1041)", EvalError::UnrepresentableText),
        (
            "StrConv(ChrW(55296),4,1033)",
            EvalError::UnsupportedExpression,
        ),
        ("StrConv(ChrW(55296),4,1/0)", EvalError::DivisionByZero),
        (
            "StrConv(IIf(True,'A',1/0),4,1041)",
            EvalError::DivisionByZero,
        ),
        ("StrConv('ヵ',8,1041)", EvalError::UnsupportedExpression),
        ("StrConv('ヸ',8,1041)", EvalError::UnsupportedExpression),
        ("StrConv('ﾠ',4,1041)", EvalError::UnsupportedExpression),
        ("StrConv('￥',8,1041)", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn width_expansion_and_pending_composition_share_cumulative_limits() {
    let expression = parse_expression("StrConv('ABｶﾞ',4,1041)", Default::default()).unwrap();
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
    let expression = parse_expression("StrConv('ABC',4,1041)", Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 3,
                ..Default::default()
            },
        )
        .evaluate(&expression),
        Err(EvalError::TextLimit)
    );
}
