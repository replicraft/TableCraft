use super::*;
use proptest::prelude::*;
use std::collections::BTreeSet;
use tablecraft_schema::SchemaName;

fn index(kind: u8, flags: u8, backing_primary: bool) -> NativeIndex {
    NativeIndex {
        name: SchemaName::new("Key".to_owned()).unwrap(),
        number: 0,
        data_number: 0,
        root: 1,
        usage: 0,
        kind,
        flags,
        backing_primary,
        columns: Vec::new(),
        reference: None,
    }
}

#[test]
fn declarations_keep_unknown_and_foreign_profiles_explicit() {
    let primary = index(1, 0x80, true).constraints().unwrap();
    assert!(primary.primary_key() && primary.unique() && primary.required());
    let alias = index(0, 0x80, true).constraints().unwrap();
    assert!(!alias.primary_key() && alias.unique() && alias.required());
    let ordinary = index(0, 0x83, false).constraints().unwrap();
    assert!(ordinary.unique() && ordinary.ignore_nulls() && !ordinary.required());
    assert!(index(2, 0x81, false).constraints().is_none());
    assert!(index(0, 0x84, false).constraints().is_none());
}

#[test]
fn unique_nulls_are_distinct_from_primary_and_required_null_rejection() {
    let unique = index(0, 0x81, false).constraints().unwrap();
    let mut previous = None;
    unique.check_key(&mut previous, None).unwrap();
    unique.check_key(&mut previous, None).unwrap();
    unique.check_key(&mut previous, Some(7)).unwrap();
    assert_eq!(
        unique.check_key(&mut previous, Some(7)),
        Err(IndexConstraintViolation::DuplicateKey)
    );
    for constraints in [index(1, 0x80, true), index(0, 0x88, false)] {
        assert_eq!(
            constraints
                .constraints()
                .unwrap()
                .check_key(&mut None, None),
            Err(IndexConstraintViolation::NullKey)
        );
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(0x434f4e535452), ..Default::default()
    })]
    #[test]
    fn ordered_integer_checks_match_independent_set_rules(
        mut keys in prop::collection::vec(prop::option::of(prop_oneof![Just(i64::MIN), Just(i64::MAX), -10_i64..10]), 0..100),
        unique in any::<bool>(), required in any::<bool>(), descending in any::<bool>(),
    ) {
        keys.sort();
        if descending { keys.reverse(); }
        let constraints = IndexConstraints { primary: false, unique, required, ignore_nulls: false };
        let expected_null = required && keys.contains(&None);
        let non_null: Vec<_> = keys.iter().flatten().copied().collect();
        let expected_duplicate = unique && non_null.iter().collect::<BTreeSet<_>>().len() != non_null.len();
        let mut previous = None;
        let result = keys.into_iter().try_for_each(|key| constraints.check_key(&mut previous, key));
        prop_assert_eq!(result.is_err(), expected_null || expected_duplicate);
    }
}
