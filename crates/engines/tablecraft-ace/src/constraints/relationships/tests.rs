use super::*;
use proptest::prelude::*;
use std::collections::BTreeSet;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_048_576, 100_000, 1_000_000)
}

#[test]
fn membership_uniqueness_and_nonunique_children_have_distinct_results() {
    assert!(validate_keys(vec![1, 2], vec![1, 1, 2], false, &mut budget(), &|| false).is_ok());
    assert!(validate_keys(vec![], vec![], true, &mut budget(), &|| false).is_ok());
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
