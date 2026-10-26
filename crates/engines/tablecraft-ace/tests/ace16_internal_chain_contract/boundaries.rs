use super::{model::*, support};
use std::cell::Cell;
use tablecraft_ace::{EditError, InspectError};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn partial_asymmetric_cross_level_and_root_links_refuse_without_a_candidate() {
    let model = database(6, false, true, false);
    let deleted = branch(&model, INTERNALS[0]);
    let expected = requests(&model, &deleted);
    let mut cases = Vec::new();
    for (page, field, target) in [
        (17, NEXT, 0),
        (17, NEXT, 23),
        (18, PREVIOUS, 0),
        (18, PREVIOUS, 23),
        (17, NEXT, LEAVES[0]),
        (18, NEXT, ROOT),
        (ROOT, PREVIOUS, 17),
        (ROOT, NEXT, 17),
    ] {
        let mut bytes = model.base.tree.bytes.clone();
        support::put32(&mut bytes, page as usize * PAGE_BYTES + field, target);
        cases.push(bytes);
    }
    let mut partial_legacy = database(6, false, false, false).base.tree.bytes;
    support::put32(&mut partial_legacy, 17 * PAGE_BYTES + NEXT, 18);
    cases.push(partial_legacy);
    for (case, bytes) in cases.iter().enumerate() {
        let before = bytes.clone();
        let result = preview(bytes, &expected);
        assert!(result.is_err(), "invalid original chain case {case}");
        assert_eq!(bytes, &before);
    }
    let result = candidate(&model, &deleted).unwrap();
    verify(&model, &result, &deleted, &[17, 15]);
}

#[test]
fn zero_internal_levels_stay_zero_while_nonempty_leaf_links_are_repaired() {
    let model = database(6, false, false, true);
    let deleted = branch(&model, INTERNALS[1]);
    let result = candidate(&model, &deleted).unwrap();
    verify(&model, &result, &deleted, &[18, 16, 19]);
    for page in [17, 23] {
        let offset = page * PAGE_BYTES + PREVIOUS;
        assert_eq!(&result.candidate_bytes()[offset..offset + 8], &[0; 8]);
    }
}

#[test]
fn quotas_cancellation_and_retry_never_publish_partial_chain_repairs() {
    let model = database(6, false, true, true);
    let deleted = branch(&model, INTERNALS[1]);
    let expected = requests(&model, &deleted);
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 20_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 20_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        let result = capture(&model.base.tree.bytes).preview_ace16_deletes(
            "Items",
            &expected,
            &mut allowance,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let observed = Cell::new(0usize);
    let complete = capture(&model.base.tree.bytes)
        .preview_ace16_deletes("Items", &expected, &mut budget(), &|| {
            observed.set(observed.get() + 1);
            false
        })
        .unwrap();
    let checkpoints = [
        0,
        1,
        observed.get() / 3,
        observed.get() * 2 / 3,
        observed.get() - 1,
    ];
    for checkpoint in checkpoints {
        let calls = Cell::new(0usize);
        let result = capture(&model.base.tree.bytes).preview_ace16_deletes(
            "Items",
            &expected,
            &mut budget(),
            &|| {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            },
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
    let retry = candidate(&model, &deleted).unwrap();
    assert_eq!(retry.candidate_bytes(), complete.candidate_bytes());
    verify(&model, &retry, &deleted, &[18, 16, 19]);
}
