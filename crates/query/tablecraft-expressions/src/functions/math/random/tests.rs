use super::*;
use proptest::prelude::*;

fn number(value: Value) -> f32 {
    let Value::Single(value) = value else {
        panic!("Single required");
    };
    value.to_float()
}

#[test]
fn omitted_positive_zero_and_negative_calls_have_distinct_state_transitions() {
    let state = RndState::default();
    assert_eq!(
        number(state.apply(Some(&Value::Int16(0))).unwrap()),
        0.01953125
    );
    let first = state.apply(None).unwrap();
    assert_eq!(number(first.clone()), 0.7055475);
    assert_eq!(state.apply(Some(&Value::Int16(0))).unwrap(), first);
    let seeded = state.apply(Some(&Value::Int16(-1))).unwrap();
    state.apply(None).unwrap();
    assert_eq!(state.apply(Some(&Value::Int16(-1))).unwrap(), seeded);
    assert_eq!(number(seeded), 0.22400701);
    assert_ne!(state.apply(Some(&Value::Int16(1))).unwrap(), first);
}

#[test]
fn raw_seed_boundaries_and_invalid_operands_do_not_advance_the_sequence() {
    assert!(matches!(
        RndState::new(RndState::MAX_SEED + 1),
        Err(EvalError::Domain)
    ));
    for argument in [
        Value::Null,
        Value::Text("1".into()),
        Value::Single(Single::from_float(f32::NAN)),
    ] {
        let state = RndState::default();
        assert!(state.apply(Some(&argument)).is_err());
        assert_eq!(state.apply(None), RndState::default().apply(None));
    }
    let state = RndState::new(RndState::MAX_SEED).unwrap();
    assert!(number(state.apply(Some(&Value::Int16(0))).unwrap()) < 1.0);
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn bounded_sequence_matches_wide_integer_modular_arithmetic(seed in 0u32..=16_777_215, count in 1usize..128) {
        let state = RndState::new(seed).unwrap();
        let mut expected = u64::from(seed);
        for _ in 0..count {
            expected = (1_140_671_485 * expected + 12_820_163) % 16_777_216;
            let value = number(state.apply(None).unwrap());
            prop_assert_eq!(value, expected as f32 / 16_777_216.0);
            prop_assert!((0.0..1.0).contains(&value));
        }
    }
}
