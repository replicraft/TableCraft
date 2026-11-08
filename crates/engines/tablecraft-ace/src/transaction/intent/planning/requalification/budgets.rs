//! Proof traversal and native reconstruction cannot borrow each other's allowance.
use super::{
    EditError, InspectError, NativeRecoveryQualificationError, Resource, ResourceBudget,
    TEST_BYTES, TEST_ROWS, TEST_STEPS, budget, image, jet4, mutation_fixture,
};
use std::cell::Cell;

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

#[test]
fn split_budget_success_keeps_original_results_and_the_single_budget_total_charge() {
    let before = mutation_fixture::database();
    let mut shared = budget();
    let shared_start = POOLS.map(|pool| shared.remaining(pool));
    let original = jet4(&before, 77)
        .requalify(image(&before), &mut shared, &|| false)
        .unwrap();

    let mut native = budget();
    let mut proof = budget();
    let native_start = POOLS.map(|pool| native.remaining(pool));
    let proof_start = POOLS.map(|pool| proof.remaining(pool));
    let separated = jet4(&before, 77)
        .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
        .unwrap();
    for (index, pool) in POOLS.into_iter().enumerate() {
        let shared_used = shared_start[index] - shared.remaining(pool);
        let native_used = native_start[index] - native.remaining(pool);
        let proof_used = proof_start[index] - proof.remaining(pool);
        assert_eq!(shared_used, native_used + proof_used, "{pool:?}");
    }
    assert_eq!(proof.remaining(Resource::Bytes), TEST_BYTES);
    assert_eq!(proof.remaining(Resource::Rows), TEST_ROWS);
    assert!(proof.remaining(Resource::Steps) < TEST_STEPS);
    assert!(native.remaining(Resource::Bytes) < TEST_BYTES);
    assert_eq!(separated.before_bytes(), original.before_bytes());
    assert_eq!(separated.after_bytes(), original.after_bytes());
    assert_eq!(separated.changed_rows(), original.changed_rows());
    assert_eq!(
        separated.intent().encode(&mut budget(), &|| false).unwrap(),
        original.intent().encode(&mut budget(), &|| false).unwrap()
    );
}

#[test]
fn exhausting_either_pool_refuses_without_transferring_work_to_the_other() {
    let before = mutation_fixture::database();
    let mut native = budget();
    let mut proof = ResourceBudget::new(0, 0, 0);
    let error = jet4(&before, 77)
        .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
        .err();
    assert!(matches!(
        error,
        Some(NativeRecoveryQualificationError::Inspect(InspectError::Limit(error)))
            if error.resource == Resource::Steps
    ));
    assert_eq!(native.remaining(Resource::Bytes), TEST_BYTES);
    assert_eq!(native.remaining(Resource::Rows), TEST_ROWS);
    assert_eq!(native.remaining(Resource::Steps), TEST_STEPS);

    let mut native = ResourceBudget::new(0, TEST_ROWS, TEST_STEPS);
    let mut proof = ResourceBudget::new(0, 0, TEST_STEPS);
    let error = jet4(&before, 77)
        .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
        .err();
    assert!(matches!(
        error,
        Some(NativeRecoveryQualificationError::Edit(EditError::Inspect(InspectError::Limit(error))))
            if error.resource == Resource::Bytes
    ));
    assert!(proof.remaining(Resource::Steps) < TEST_STEPS);

    let mut native = ResourceBudget::new(TEST_BYTES, TEST_ROWS, 0);
    let mut proof = ResourceBudget::new(0, 0, TEST_STEPS);
    let error = jet4(&before, 77)
        .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
        .err();
    assert!(matches!(
        error,
        Some(NativeRecoveryQualificationError::Edit(EditError::Inspect(InspectError::Limit(error))))
            if error.resource == Resource::Steps
    ));
    assert!(proof.remaining(Resource::Steps) < TEST_STEPS);
}

#[test]
fn cancellation_wins_before_empty_pools_and_prevents_final_capability_publication() {
    let before = mutation_fixture::database();
    let mut native = ResourceBudget::new(0, 0, 0);
    let mut proof = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        jet4(&before, 77)
            .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| true,),
        Err(NativeRecoveryQualificationError::Inspect(
            InspectError::Cancelled
        ))
    ));

    let polls = Cell::new(0_usize);
    jet4(&before, 77)
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let last = polls.get();
    assert!(last > 1);
    polls.set(0);
    assert!(matches!(
        jet4(&before, 77).requalify_with_budgets(
            image(&before),
            &mut budget(),
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == last
            },
        ),
        Err(NativeRecoveryQualificationError::Inspect(
            InspectError::Cancelled
        ))
    ));
    assert_eq!(polls.get(), last);
}
