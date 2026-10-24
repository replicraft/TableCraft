use super::*;
use crate::{evaluation::NoBindings, predicates::LikeMode};
use tablecraft_core::{limits::ResourceBudget, values::ExpressionString};
use tablecraft_sql::parse_expression;

#[test]
fn raw_like_null_requires_mode_without_decoding_or_collation() {
    for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
        for raw in ["ChrW(55296)", "ChrB(65)", "CStr(LeftB('😀',1))"] {
            for sql in [
                format!("{raw} LIKE NULL"),
                format!("NULL LIKE {raw}"),
                format!("{raw} NOT LIKE NULL"),
                format!("NULL NOT LIKE {raw}"),
            ] {
                let expression = parse_expression(&sql, Default::default()).unwrap();
                let mut budget = ResourceBudget::new(65536, 0, 10000);
                let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
                    .with_like_mode(mode)
                    .evaluate(&expression);
                assert_eq!(result, Ok(Value::Null), "{sql}");
            }
        }
    }

    let raw = ExpressionValue::StringBytes(ExpressionString::from_owned(vec![65]));
    let null = ExpressionValue::Value(Value::Null);
    let mut budget = ResourceBudget::new(0, 0, 2);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.apply_expression_like(&raw, &null, false),
        Err(EvalError::LikeModeRequired)
    );
    assert_eq!(
        evaluator
            .with_like_mode(LikeMode::Ansi89)
            .apply_expression_like(&null, &raw, true),
        Ok(Value::Null)
    );

    let mut budget = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_like_mode(LikeMode::Ansi89)
            .apply_expression_like(&raw, &null, false),
        Err(EvalError::Limit(_))
    ));

    let mut budget = ResourceBudget::new(0, 0, 1);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_like_mode(LikeMode::Ansi89)
            .with_cancellation(&|| true)
            .apply_expression_like(&raw, &null, false),
        Err(EvalError::Cancelled)
    );
}

#[test]
fn raw_like_keeps_eager_and_nonnull_publication_failures() {
    for (sql, expected) in [
        ("ChrW(55296) LIKE 1/0", EvalError::DivisionByZero),
        ("NULL LIKE ChrW(65536)", EvalError::Domain),
        ("ChrW(55296) LIKE '*'", EvalError::UnrepresentableText),
        ("'*' LIKE ChrB(65)", EvalError::UnrepresentableText),
    ] {
        let expression = parse_expression(sql, Default::default()).unwrap();
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_like_mode(LikeMode::Ansi89)
            .evaluate(&expression);
        assert_eq!(result, Err(expected), "{sql}");
    }
}
