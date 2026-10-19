use super::*;
use crate::ConstraintViolation;
use proptest::prelude::*;
use std::collections::BTreeSet;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_048_576, 100_000, 1_000_000)
}

#[test]
fn membership_uniqueness_and_nonunique_children_have_distinct_results() {
    assert!(validate_keys(vec![1, 2], vec![1, 1, 2], false, &mut budget(), &|| false).is_ok());
    assert!(validate_keys(Vec::<i32>::new(), vec![], true, &mut budget(), &|| false).is_ok());
    assert_eq!(
        validate_keys(vec![1], vec![2], false, &mut budget(), &|| false),
        Err(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
    assert_eq!(
        validate_keys(vec![1, 1], vec![], false, &mut budget(), &|| false),
        Err(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
    assert_eq!(
        validate_keys(vec![1], vec![1, 1], true, &mut budget(), &|| false),
        Err(InspectError::Constraint(
            ConstraintViolation::DuplicateChild
        ))
    );
    assert!(matches!(
        validate_keys(vec![1], vec![1], false, &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        validate_keys(
            vec![1],
            vec![1],
            false,
            &mut ResourceBudget::new(1000, 1000, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x464f524549474e),..Default::default()})]
    #[test]
    fn sorted_key_validation_matches_an_independent_set_model(parents in prop::collection::vec(-20i32..20,0..40),children in prop::collection::vec(-20i32..20,0..40),one_to_one in any::<bool>()) {
        let parent_set=parents.iter().copied().collect::<BTreeSet<_>>();
        let child_set=children.iter().copied().collect::<BTreeSet<_>>();
        let expected=if parent_set.len()!=parents.len() {Err(InspectError::Constraint(ConstraintViolation::DuplicateParent))}
            else if one_to_one&&child_set.len()!=children.len() {Err(InspectError::Constraint(ConstraintViolation::DuplicateChild))}
            else if children.iter().any(|child|!parent_set.contains(child)) {Err(InspectError::Constraint(ConstraintViolation::MissingParent))}
            else {Ok(())};
        prop_assert_eq!(validate_keys(parents,children,one_to_one,&mut budget(),&||false),expected);
    }
}

fn tuple(a: Option<i64>, b: Option<i64>) -> [Option<i64>; 10] {
    let mut key = [None; 10];
    key[0] = a;
    key[1] = b;
    key
}
#[test]
fn partially_null_keys_require_exact_parents_and_do_not_match_componentwise() {
    let parents = vec![
        tuple(Some(1), Some(10)),
        tuple(None, Some(30)),
        tuple(Some(4), None),
    ];
    assert!(
        validate_keys(
            parents.clone(),
            vec![tuple(None, Some(30)), tuple(Some(4), None)],
            false,
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    for key in [
        tuple(Some(1), Some(30)),
        tuple(None, Some(10)),
        tuple(Some(1), None),
    ] {
        assert_eq!(
            validate_keys(parents.clone(), vec![key], false, &mut budget(), &|| false),
            Err(InspectError::Constraint(ConstraintViolation::MissingParent))
        );
    }
    assert_eq!(
        validate_keys(
            vec![tuple(None, Some(30)); 2],
            vec![],
            false,
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}

#[test]
fn int64_membership_keeps_extremes_and_adjacent_large_values_distinct() {
    let parents = vec![
        tuple(Some(i64::MIN), Some(i64::MAX)),
        tuple(None, Some(9_007_199_254_740_993)),
    ];
    assert!(
        validate_keys(
            parents.clone(),
            parents.clone(),
            true,
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    assert_eq!(
        validate_keys(
            parents,
            vec![tuple(None, Some(9_007_199_254_740_992))],
            false,
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
}

proptest! {
    #![proptest_config(ProptestConfig {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x434f4d504f534954),..Default::default()})]
    #[test]
    fn composite_checks_match_independent_option_tuple_sets(
        parents in prop::collection::vec((prop::option::of(prop_oneof![Just(i64::MIN), Just(i64::MAX), -5i64..5]),prop::option::of(-5i64..5)),0..30),
        children in prop::collection::vec((prop::option::of(prop_oneof![Just(i64::MIN), Just(i64::MAX), -5i64..5]),prop::option::of(-5i64..5)),0..30),
        one_to_one in any::<bool>()) {
        let parents=parents.into_iter().filter(|p|*p!=(None,None)).collect::<Vec<_>>();
        let children=children.into_iter().filter(|p|*p!=(None,None)).collect::<Vec<_>>();
        let parent_set=parents.iter().copied().collect::<BTreeSet<_>>();
        let child_set=children.iter().copied().collect::<BTreeSet<_>>();
        let expected=if parent_set.len()!=parents.len(){Err(InspectError::Constraint(ConstraintViolation::DuplicateParent))}
            else if one_to_one&&child_set.len()!=children.len(){Err(InspectError::Constraint(ConstraintViolation::DuplicateChild))}
            else if children.iter().any(|c|!parent_set.contains(c)){Err(InspectError::Constraint(ConstraintViolation::MissingParent))}
            else{Ok(())};
        let parents=parents.into_iter().map(|(a,b)|tuple(a,b)).collect();
        let children=children.into_iter().map(|(a,b)|tuple(a,b)).collect();
        prop_assert_eq!(validate_keys(parents,children,one_to_one,&mut budget(),&||false),expected);
    }
}
