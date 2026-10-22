use super::*;
use proptest::prelude::*;

fn apply_to_text(
    function: TextFunction,
    text: &str,
    positions: &[i64],
) -> Result<Value, EvalError> {
    let mut arguments = vec![Value::Text(text.into())];
    arguments.extend(positions.iter().copied().map(Value::Int64));
    Ok(match apply(function, &arguments, || Ok(()))? {
        TextResult::Null => Value::Null,
        TextResult::Comparison(value) | TextResult::CharacterCode(value) => Value::Int16(value),
        TextResult::Length(length) | TextResult::Position(length) => Value::Int32(length),
        TextResult::Text(text) => Value::Text(text.into()),
        TextResult::Character(character) => Value::Text(character.to_string()),
        TextResult::Casing(_)
        | TextResult::Replacement(_)
        | TextResult::Construction(_)
        | TextResult::NumericPrefix(_)
        | TextResult::NumericFormat(_)
        | TextResult::Partition(_)
        | TextResult::DateFormat(_)
        | TextResult::DateTimeFormat(_)
        | TextResult::CalendarName(_)
        | TextResult::Format(_)
        | TextResult::StyledNumber(_) => {
            unreachable!("helper only calls borrowed text operations")
        }
    })
}
#[test]
fn text_functions_use_utf16_units_preserve_null_and_trim_only_spaces() {
    for (function, text, positions, expected) in [
        (TextFunction::Len, "é中🦀\0", &[][..], Value::Int32(5)),
        (
            TextFunction::Trim,
            "  \té \u{a0}  ",
            &[],
            Value::Text("\té \u{a0}".into()),
        ),
        (TextFunction::LTrim, "  x  ", &[], Value::Text("x  ".into())),
        (TextFunction::RTrim, "  x  ", &[], Value::Text("  x".into())),
        (TextFunction::Left, "é🦀中", &[3], Value::Text("é🦀".into())),
        (
            TextFunction::Right,
            "é🦀中",
            &[3],
            Value::Text("🦀中".into()),
        ),
        (
            TextFunction::Mid,
            "é🦀中",
            &[2, 2],
            Value::Text("🦀".into()),
        ),
        (TextFunction::Mid, "é🦀中", &[2], Value::Text("🦀中".into())),
        (
            TextFunction::Mid,
            "é🦀中",
            &[3, 0],
            Value::Text(String::new()),
        ),
        (
            TextFunction::Mid,
            "é🦀中",
            &[i32::MAX as i64],
            Value::Text(String::new()),
        ),
        (
            TextFunction::Left,
            "abc",
            &[i32::MAX as i64],
            Value::Text("abc".into()),
        ),
        (TextFunction::Right, "abc", &[0], Value::Text(String::new())),
    ] {
        assert_eq!(apply_to_text(function, text, positions), Ok(expected));
        let mut arguments = vec![Value::Null];
        arguments.extend(positions.iter().copied().map(Value::Int64));
        assert_eq!(apply(function, &arguments, || Ok(())), Ok(TextResult::Null));
    }
    for (function, positions) in [
        (TextFunction::Left, vec![2]),
        (TextFunction::Right, vec![2]),
        (TextFunction::Mid, vec![3, 1]),
    ] {
        assert_eq!(
            apply_to_text(function, "é🦀中", &positions),
            Err(EvalError::UnrepresentableText)
        );
    }
}
#[test]
fn text_arguments_domains_and_metered_scans_fail_explicitly() {
    for (function, positions) in [
        (TextFunction::Left, vec![-1]),
        (TextFunction::Right, vec![-1]),
        (TextFunction::Mid, vec![0]),
        (TextFunction::Mid, vec![1, -1]),
    ] {
        assert_eq!(
            apply_to_text(function, "abc", &positions),
            Err(EvalError::Domain)
        );
    }
    assert_eq!(
        apply_to_text(TextFunction::Left, "abc", &[i64::MAX]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        apply(TextFunction::Len, &[Value::Int32(4)], || Ok(())),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        apply(
            TextFunction::Left,
            &[Value::Text("abc".into()), Value::Null],
            || Ok(())
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        apply(TextFunction::Mid, &[], || Ok(())),
        Err(EvalError::UnsupportedExpression)
    );
    let arguments = [Value::Text("long text".into())];
    let mut visits = 0;
    assert_eq!(
        apply(TextFunction::Len, &arguments, || {
            visits += 1;
            if visits == 3 {
                Err(EvalError::Cancelled)
            } else {
                Ok(())
            }
        }),
        Err(EvalError::Cancelled)
    );
    assert_eq!(visits, 3);
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn slices_match_an_independent_utf16_buffer_model(text in prop::collection::vec(any::<char>(),0..32), start in 1_i64..70, length in 0_i64..70) {
        let text: String = text.into_iter().collect();
        let units: Vec<u16> = text.encode_utf16().collect();
        for (function, positions, from, to) in [
            (TextFunction::Left,vec![length],0,(length as usize).min(units.len())),
            (TextFunction::Right,vec![length],units.len().saturating_sub(length as usize),units.len()),
            (TextFunction::Mid,vec![start,length],((start-1) as usize).min(units.len()),((start-1+length) as usize).min(units.len())),
        ] {
            let expected = String::from_utf16(&units[from..to]).map(Value::Text).map_err(|_| EvalError::UnrepresentableText);
            prop_assert_eq!(apply_to_text(function,&text,&positions),expected);
        }
        prop_assert_eq!(apply_to_text(TextFunction::Len,&text,&[]),Ok(Value::Int32(units.len() as i32)));
    }
}
