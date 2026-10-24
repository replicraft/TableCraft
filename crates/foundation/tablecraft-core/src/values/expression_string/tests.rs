use super::*;
use crate::values::{ExpressionState, ExpressionValue, Value, ValueError};
use proptest::prelude::*;

#[test]
fn cloning_and_representation_equality_use_only_active_string_bytes() {
    let selected = ExpressionString::from_owned(vec![255, 65, 0, 255])
        .select(1..3)
        .unwrap();
    let owned = selected.clone();
    assert_eq!(owned.bytes, &[65, 0]);
    assert_eq!(owned.range, 0..2);
    assert_eq!(owned, selected);

    let expression = ExpressionValue::StringBytes(selected);
    assert!(matches!(
        Value::try_from(expression),
        Err(ValueError::ExpressionNotStorable(
            ExpressionState::StringBytes
        ))
    ));
}

#[test]
fn adoption_and_nested_ranges_retain_the_allocation_and_exact_bytes() {
    let bytes = vec![0, 216, 65, 0, 255];
    let selected_pointer = bytes[2..4].as_ptr();
    let value = ExpressionString::from_owned(bytes)
        .select(1..5)
        .unwrap()
        .select(1..3)
        .unwrap();

    assert_eq!(value.as_bytes(), &[65, 0]);
    assert_eq!(value.as_bytes().as_ptr(), selected_pointer);
    assert_eq!(value.len(), 2);
}

#[test]
fn odd_surrogate_and_empty_ranges_do_not_imply_unicode_or_null() {
    let value = ExpressionString::from_owned(vec![0, 216, 65]);
    assert_eq!(value.as_bytes(), &[0, 216, 65]);
    assert_eq!(format!("{value:?}"), "ExpressionString { len: 3, .. }");

    let empty = value.select(3..3).unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.as_bytes(), &[]);
}

#[test]
fn invalid_ranges_are_rejected_without_payload_diagnostics() {
    for (start, end) in [(2, 1), (0, 4), (usize::MAX, usize::MAX)] {
        let value = ExpressionString::from_owned(vec![65, 0, 66]);
        assert!(matches!(
            value.select(start..end),
            Err(ExpressionStringRangeError)
        ));
    }
    assert_eq!(
        ExpressionStringRangeError.to_string(),
        "invalid expression string byte range"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5354_5249_4e47_5241),
        failure_persistence: None,
        ..Default::default()
    })]

    #[test]
    fn selected_views_match_independent_slice_contents(
        bytes in proptest::collection::vec(any::<u8>(), 0..128),
        first in any::<usize>(),
        second in any::<usize>(),
    ) {
        let first = first % (bytes.len() + 1);
        let second = second % (bytes.len() + 1);
        let range = first.min(second)..first.max(second);
        let expected = bytes[range.clone()].to_vec();
        let selected = ExpressionString::from_owned(bytes).select(range).unwrap();

        prop_assert_eq!(selected.as_bytes(), expected);
    }
}
