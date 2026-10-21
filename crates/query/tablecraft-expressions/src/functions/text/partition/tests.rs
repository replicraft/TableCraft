use super::*;
use crate::evaluation::{EvalLimits, Evaluator, NoBindings};
use crate::functions::text::TextFunction;
use proptest::prelude::*;
use tablecraft_core::{limits::ResourceBudget, values::Double};

fn render(arguments: &[Value]) -> Result<Value, EvalError> {
    let mut budget = ResourceBudget::new(1000, 1000, 1000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .apply_text_function(TextFunction::Partition, arguments)
}

fn call(values: [i32; 4]) -> Result<Value, EvalError> {
    render(&values.map(Value::Int32))
}

#[test]
fn boundary_goldens_preserve_spaces_empty_sides_and_short_final_buckets() {
    for (arguments, expected) in [
        ([-1, 0, 99, 5], "   : -1"),
        ([0, 0, 99, 5], "  0:  4"),
        ([99, 0, 99, 5], " 95: 99"),
        ([100, 0, 99, 5], "100:   "),
        ([19, 20, 199, 10], "   : 19"),
        ([20, 20, 199, 10], " 20: 29"),
        ([199, 20, 199, 10], "190:199"),
        ([200, 20, 199, 10], "200:   "),
        ([99, 100, 1010, 20], "    :  99"),
        ([100, 100, 1010, 20], " 100: 119"),
        ([1010, 100, 1010, 20], "1000:1010"),
        ([1011, 100, 1010, 20], "1011:    "),
        ([-1, 0, 1, 2], "  :-1"),
        ([100, 0, 1000, 1], " 100: 100"),
        ([999, 0, 9, 1], "999:999"),
        ([i32::MAX, 0, i32::MAX, i32::MAX], "2147483647:2147483647"),
        ([i32::MIN, 0, 1, 1], "-2147483648:-2147483648"),
    ] {
        assert_eq!(
            call(arguments),
            Ok(Value::Text(expected.into())),
            "{arguments:?}"
        );
    }
}

#[test]
fn null_rounding_domains_and_carrier_boundaries_are_explicit() {
    assert_eq!(
        render(&[
            Value::Double(Double::from_float(2.5)),
            Value::Byte(0),
            Value::Byte(9),
            Value::Byte(2)
        ]),
        Ok(Value::Text(" 2: 3".into()))
    );

    for index in 0..4 {
        let mut arguments = [
            Value::Int32(2),
            Value::Int32(0),
            Value::Int32(9),
            Value::Int32(2),
        ];
        arguments[index] = Value::Null;
        assert_eq!(render(&arguments), Ok(Value::Null));
    }
    for arguments in [[0, -1, 9, 2], [0, 0, 0, 2], [0, 1, 0, 2], [0, 0, 9, 0]] {
        assert_eq!(call(arguments), Err(EvalError::Domain));
    }
    assert_eq!(
        render(&[
            Value::Int64(i64::MAX),
            Value::Byte(0),
            Value::Byte(9),
            Value::Byte(2)
        ]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        render(&[
            Value::Text("2".into()),
            Value::Byte(0),
            Value::Byte(9),
            Value::Byte(2)
        ]),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(render(&[]), Err(EvalError::UnsupportedExpression));
}

#[test]
fn output_bytes_work_and_cancellation_are_charged_before_publication() {
    let arguments = [0, 0, 99, 5].map(Value::Int32);
    for (bytes, steps) in [(0, 100), (100, 0)] {
        let mut budget = ResourceBudget::new(bytes, 100, steps);
        let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .apply_text_function(TextFunction::Partition, &arguments);
        assert!(matches!(result, Err(EvalError::Limit(_))));
    }

    let mut budget = ResourceBudget::new(100, 100, 100);
    let result = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 6,
            ..Default::default()
        },
    )
    .apply_text_function(TextFunction::Partition, &arguments);
    assert_eq!(result, Err(EvalError::TextLimit));

    let mut budget = ResourceBudget::new(100, 100, 100);
    let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_cancellation(&|| true)
        .apply_text_function(TextFunction::Partition, &arguments);
    assert_eq!(result, Err(EvalError::Cancelled));
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn labels_match_independently_enumerated_small_buckets(number in -10_i32..300, start in 0_i32..40, span in 1_i32..100, interval in 1_i32..30) {
        let stop = start + span;
        let expected = if interval == 1 {
            (Some(number), Some(number))
        } else if number < start {
            (None, Some(start - 1))
        } else if number > stop {
            (Some(stop + 1), None)
        } else {
            let lower = (start..=stop).step_by(interval as usize)
                .find(|lower| (*lower..=(*lower + interval - 1).min(stop)).contains(&number))
                .unwrap();
            (Some(lower), Some((lower + interval - 1).min(stop)))
        };
        let Value::Text(label) = call([number, start, stop, interval]).unwrap() else {
            unreachable!();
        };
        let (lower, upper) = label.split_once(':').unwrap();
        let actual = (lower.trim().parse::<i32>().ok(), upper.trim().parse::<i32>().ok());

        prop_assert_eq!(actual, expected);
    }
}
