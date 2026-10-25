use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::parse_expression;

#[test]
fn japanese_word_expansion_preserves_intermediate_limits_and_every_cancellation_point() {
    // The seven-byte source expands to eight bytes under proper case. A later
    // narrowing would make it six bytes, but cannot waive the intermediate cap.
    for code in [3, 11] {
        let expression =
            parse_expression(&format!("StrConv('ab　ǰ',{code},1041)"), Default::default()).unwrap();
        for maximum in [7, 8] {
            let mut budget = ResourceBudget::new(65_536, 0, 10_000);
            let result = Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: maximum,
                    ..Default::default()
                },
            )
            .evaluate(&expression);
            let expected = if maximum == 7 {
                Err(EvalError::TextLimit)
            } else if code == 3 {
                Ok(Value::Text("Ab　J\u{30c}".into()))
            } else {
                Ok(Value::Text("Ab J\u{30c}".into()))
            };
            assert_eq!(result, expected);
        }
    }

    let expression = parse_expression("StrConv('ab　ǰ',11,1041)", Default::default()).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_cancellation(&count)
        .evaluate(&expression)
        .unwrap();
    assert!(observed.get() > 0);
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&cancelled)
                .evaluate(&expression),
            Err(EvalError::Cancelled)
        );
    }
}
