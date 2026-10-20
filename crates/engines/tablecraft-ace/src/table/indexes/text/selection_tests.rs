use super::{selection::EncodedSelection, *};
use crate::table::indexes::key::TestKeyProfile;
use proptest::prelude::*;
use std::ops::Bound::{Excluded, Included, Unbounded};

fn select(profile: &mut TextProfile, selection: TextIndexSelection<'_>) {
    let mut budget = tablecraft_core::limits::ResourceBudget::new(1_000_000, 100, 100_000);
    profile
        .select(
            selection,
            &mut KeyWork {
                budget: &mut budget,
                cancelled: &|| false,
            },
        )
        .unwrap();
}

#[test]
fn native_equivalent_bounds_preserve_exclusion_null_and_physical_direction() {
    for descending in [false, true] {
        let mut policy = super::tests::profile(0, descending);
        let a = policy.row_key_test(&[Value::Text("a".into())]).unwrap();
        let null = policy.row_key_test(&[Value::Null]).unwrap();
        select(
            &mut policy,
            TextIndexSelection::Range {
                start: Included("A  "),
                end: Included("a"),
            },
        );
        assert!(policy.contains(&a));
        assert!(!policy.contains(&null));
        assert!(!policy.complete());
        select(
            &mut policy,
            TextIndexSelection::Range {
                start: Excluded("A  "),
                end: Included("a"),
            },
        );
        assert!(!policy.contains(&a));
        assert!(!policy.may_match(None, None));
        let (first, last) = if descending { ("z", "a") } else { ("a", "z") };
        select(
            &mut policy,
            TextIndexSelection::Range {
                start: Included(first),
                end: Included(last),
            },
        );
        assert!(policy.contains(&a));
        select(
            &mut policy,
            TextIndexSelection::Range {
                start: Included(last),
                end: Included(first),
            },
        );
        assert!(!policy.may_match(None, None));
        select(&mut policy, TextIndexSelection::Null);
        assert!(policy.contains(&null));
        assert!(!policy.contains(&a));
        select(&mut policy, TextIndexSelection::All);
        assert!(policy.complete() && policy.contains(&null) && policy.contains(&a));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, rng_seed: proptest::test_runner::RngSeed::Fixed(0x544558545345454b), ..Default::default() })]
    #[test]
    fn inclusive_hulls_never_prune_a_selected_key(
        mut keys in prop::collection::vec(prop::collection::vec(any::<u8>(), 1..40), 5),
        start_mode in 0..3_u8, end_mode in 0..3_u8,
    ) {
        // Arbitrary opaque physical bytes test the interval math independently
        // of the character encoder. Sorted enclosing fences need not be live keys.
        keys.sort();
        let candidate = &keys[2];
        let make = |mode, bytes: Vec<u8>| match mode { 0 => Unbounded, 1 => Included(TextKey(bytes)), _ => Excluded(TextKey(bytes)) };
        let selection = EncodedSelection::Range { start: make(start_mode, keys[1].clone()), end: make(end_mode, keys[3].clone()) };
        let lower_ok = start_mode == 0 || if start_mode == 1 { candidate >= &keys[1] } else { candidate > &keys[1] };
        let upper_ok = end_mode == 0 || if end_mode == 1 { candidate <= &keys[3] } else { candidate < &keys[3] };
        let expected = candidate.as_slice() != [0] && lower_ok && upper_ok;
        prop_assert_eq!(selection.contains(candidate, 0), expected);
        if expected { prop_assert!(selection.may_match(Some(&keys[0]), Some(&keys[4]), 0)); }
    }
}
