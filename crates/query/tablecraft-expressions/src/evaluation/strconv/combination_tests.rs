use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65_536, 0, 100_000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expression)
}

#[test]
fn combined_flags_apply_case_then_width_and_script_without_implicit_encoding() {
    for (code, expected) in [
        (5, "ＡＢ　ガ　が"),
        (6, "ａｂ　ガ　が"),
        (7, "Ａｂ　ガ　が"),
        (9, "AB ｶﾞ が"),
        (17, "AB ｶﾞ ガ"),
        (18, "ab ｶﾞ ガ"),
        (19, "Ab ｶﾞ ガ"),
        (20, "ａＢ　ガ　ガ"),
        (21, "ＡＢ　ガ　ガ"),
        (24, "aB ｶﾞ ｶﾞ"),
        (27, "Ab ｶﾞ ｶﾞ"),
        (33, "AB ｶﾞ が"),
        (36, "ａＢ　が　が"),
        (37, "ＡＢ　が　が"),
        (40, "aB ｶﾞ が"),
        (43, "Ab ｶﾞ が"),
    ] {
        let sql = format!("StrConv('aB ｶﾞ が',{code},1041)");
        assert_eq!(evaluate(&sql), Ok(Value::Text(expected.into())), "{sql}");
    }
    assert_eq!(
        evaluate("StrConv('hello WORLD',7,1041)"),
        Ok(Value::Text("Ｈｅｌｌｏ　Ｗｏｒｌｄ".into()))
    );
    assert_eq!(
        evaluate("StrConv('ﬁ ß',5,1041)"),
        Ok(Value::Text("ＦＩ　ＳＳ".into()))
    );
    assert_eq!(
        evaluate("StrConv('ａＢ　ガ が',10,1041)"),
        Ok(Value::Text("ab ｶﾞ が".into()))
    );
    assert_eq!(
        evaluate("StrConv(MidB('aaｶﾞ',5,4),37,1041)"),
        Ok(Value::Text("が".into()))
    );
    assert_eq!(
        evaluate("StrConv('ｶﾞ',40,1041)"),
        Ok(Value::Text("ｶﾞ".into()))
    );
    assert_eq!(
        evaluate("StrConv('ガ',40,1041)"),
        Ok(Value::Text("が".into()))
    );
}

#[test]
fn combination_admission_preserves_exact_locale_raw_state_and_eager_errors() {
    for code in [5, 7, 10, 17, 20, 24, 37, 40] {
        assert_eq!(
            evaluate(&format!("StrConv(NULL,{code},1041)")),
            Ok(Value::Null)
        );
        for locale in ["1033", "66577"] {
            assert_eq!(
                evaluate(&format!("StrConv(NULL,{code},{locale})")),
                Err(EvalError::UnsupportedExpression)
            );
        }
        assert_eq!(
            evaluate(&format!("StrConv('x',{code})")),
            Err(EvalError::UnsupportedExpression)
        );
        assert_eq!(
            evaluate(&format!("StrConv('x',{code},NULL)")),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            evaluate(&format!("StrConv(ChrW(55296),{code},1041)")),
            Err(EvalError::UnrepresentableText)
        );
        assert_eq!(
            evaluate(&format!("StrConv(ChrW(55296),{code},1/0)")),
            Err(EvalError::DivisionByZero)
        );
    }
    for code in [12, 48, 192, 256, -1] {
        assert_eq!(
            evaluate(&format!("StrConv(NULL,{code},1041)")),
            Err(EvalError::Domain)
        );
    }
    for code in [68, 80, 100, 132, 164] {
        assert_eq!(
            evaluate(&format!("StrConv(NULL,{code},1041)")),
            Err(EvalError::UnsupportedExpression)
        );
    }
}

#[test]
fn all_case_width_and_kana_flag_combinations_have_explicit_admission() {
    let accepted = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 32,
        33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 64, 65, 66, 67, 128, 129, 130, 131,
    ];
    for code in 0..=255 {
        assert_eq!(
            Operation::from_code(code).is_ok(),
            accepted.contains(&code),
            "{code}"
        );
    }
    // The independently usable legacy component recipe still owns case/encoding
    // only; evaluator composition does not change that public contract.
    assert_eq!(
        strconv::Conversion::from_code(5),
        Err(EvalError::UnsupportedExpression)
    );
    assert_eq!(
        strconv::Conversion::from_code(36),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn every_combined_stage_uses_one_budget_and_observed_cancellation_pool() {
    let expression = parse_expression("StrConv('aｶﾞ ガ あ',43,1041)", Default::default()).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65_536, 0, 100_000);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_cancellation(&count)
        .evaluate(&expression);
    assert_eq!(result, Ok(Value::Text("Aｶﾞ が あ".into())));
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65_536, 0, 100_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&cancelled)
                .evaluate(&expression),
            Err(EvalError::Cancelled)
        );
    }
    let mut budget = ResourceBudget::new(0, 0, 100_000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expression),
        Err(EvalError::Limit(_))
    ));
}
