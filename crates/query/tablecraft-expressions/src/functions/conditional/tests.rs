use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double};

fn apply(function: ConditionalFunction, values: Vec<Value>) -> Result<Value, EvalError> {
    let mut selection = function.start(values.len())?;
    for value in values {
        selection.push(value)?;
    }
    selection.finish()
}
#[test]
fn switch_chooses_first_true_pair_without_coercing_or_replacing_its_null_value() {
    use ConditionalFunction::Switch;
    assert_eq!(
        apply(
            Switch,
            vec![
                Value::Boolean(false),
                Value::Int16(1),
                Value::Boolean(true),
                Value::Text("chosen".into()),
                Value::Boolean(true),
                Value::Int32(2)
            ]
        ),
        Ok(Value::Text("chosen".into()))
    );
    assert_eq!(
        apply(
            Switch,
            vec![
                Value::Boolean(true),
                Value::Null,
                Value::Boolean(true),
                Value::Int16(2)
            ]
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        apply(
            Switch,
            vec![
                Value::Null,
                Value::Int16(1),
                Value::Boolean(false),
                Value::Int16(2)
            ]
        ),
        Ok(Value::Null)
    );
    for condition in [
        Value::Int64(i64::MIN),
        Value::Currency(Currency::from_scaled(1)),
        Value::Decimal(Decimal::new(1, 28).unwrap()),
        Value::Double(Double::from_float(f64::from_bits(1))),
    ] {
        assert_eq!(
            apply(Switch, vec![condition, Value::Binary(vec![0, 255])]),
            Ok(Value::Binary(vec![0, 255]))
        );
    }
}
#[test]
fn choose_uses_one_based_half_even_index_and_preserves_choice_carriers() {
    use ConditionalFunction::Choose;
    for (index, expected) in [
        (5, None),
        (15, Some(2)),
        (25, Some(2)),
        (35, Some(4)),
        (-5, None),
        (45, Some(4)),
    ] {
        for value in [
            Value::Decimal(Decimal::new(index, 1).unwrap()),
            Value::Double(Double::from_float(index as f64 / 10.0)),
        ] {
            let result = apply(
                Choose,
                vec![
                    value,
                    Value::Int16(1),
                    Value::Int16(2),
                    Value::Int16(3),
                    Value::Int16(4),
                ],
            );
            assert_eq!(result, Ok(expected.map_or(Value::Null, Value::Int16)));
        }
    }
    let choice = Value::Decimal(Decimal::new(12345678901234567890123456789, 7).unwrap());
    assert_eq!(
        apply(
            Choose,
            vec![Value::Byte(1), choice.clone(), Value::Int16(0)]
        ),
        Ok(choice)
    );
    for index in [
        Value::Null,
        Value::Int64(i64::MAX),
        Value::Decimal(Decimal::new(Decimal::MAX_COEFFICIENT, 0).unwrap()),
        Value::Boolean(true),
    ] {
        assert_eq!(apply(Choose, vec![index, Value::Int16(1)]), Ok(Value::Null));
    }
    assert_eq!(
        apply(
            Choose,
            vec![Value::Double(Double::from_float(f64::MAX)), Value::Int16(1)]
        ),
        Err(EvalError::Overflow)
    );
}
#[test]
fn invalid_conditions_are_deferred_and_not_hidden_by_an_earlier_selection() {
    let mut selection = ConditionalFunction::Switch.start(4).unwrap();
    for value in [
        Value::Boolean(true),
        Value::Int16(7),
        Value::Text("not numeric".into()),
        Value::Int16(9),
    ] {
        selection.push(value).unwrap();
    }
    assert_eq!(selection.finish(), Err(EvalError::TypeMismatch));
    for function in [ConditionalFunction::Switch, ConditionalFunction::Choose] {
        assert_eq!(
            apply(
                function,
                vec![Value::Double(Double::from_float(f64::NAN)), Value::Int16(1)]
            ),
            Err(EvalError::NonFinite)
        );
        assert!(function.start(0).is_err());
        assert!(function.start(1).is_err());
        let mut incomplete = function.start(2).unwrap();
        incomplete.push(Value::Int16(1)).unwrap();
        assert_eq!(incomplete.finish(), Err(EvalError::UnsupportedExpression));
    }
    assert!(ConditionalFunction::Switch.start(3).is_err());
    let mut extra = ConditionalFunction::Choose.start(2).unwrap();
    extra.push(Value::Int16(1)).unwrap();
    extra.push(Value::Int16(8)).unwrap();
    assert_eq!(
        extra.push(Value::Int16(9)),
        Err(EvalError::UnsupportedExpression)
    );
    assert_eq!(extra.finish(), Err(EvalError::UnsupportedExpression));
    assert_eq!(
        ConditionalFunction::resolve("sWitCh"),
        Some(ConditionalFunction::Switch)
    );
    assert_eq!(
        ConditionalFunction::resolve("CHOOSE"),
        Some(ConditionalFunction::Choose)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn generated_switches_and_choices_match_independent_vector_selection(pairs in prop::collection::vec((any::<bool>(),any::<i32>()),1..40),index in -10_i16..50){
        let expected=pairs.iter().find(|(condition,_)|*condition).map_or(Value::Null,|(_,value)|Value::Int32(*value));
        let values=pairs.iter().flat_map(|(condition,value)|[Value::Boolean(*condition),Value::Int32(*value)]).collect();
        prop_assert_eq!(apply(ConditionalFunction::Switch,values),Ok(expected));
        let expected=usize::try_from(index).ok().and_then(|n|n.checked_sub(1)).and_then(|n|pairs.get(n)).map_or(Value::Null,|(_,value)|Value::Int32(*value));
        let values=std::iter::once(Value::Int16(index)).chain(pairs.into_iter().map(|(_,value)|Value::Int32(value))).collect();
        prop_assert_eq!(apply(ConditionalFunction::Choose,values),Ok(expected));
    }
}
