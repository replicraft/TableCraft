use super::*;
use std::cell::Cell;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn every_resource_pool_and_sampled_late_cancellation_prevent_publication() {
    let model = model::binary(false, true);
    let updates = [SlotScalarUpdate::new(
        4,
        Value::Binary((1..=8).collect()),
        Value::Binary((1..=9).collect()),
    )];
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        let result = capture(&model.bytes).preview_ace16_page_updates(
            "Items",
            "Score",
            &updates,
            &mut allowance,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0usize);
    let expected = capture(&model.bytes)
        .preview_ace16_page_updates("Items", "Score", &updates, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let observed = calls.get();
    assert!(observed > 8);
    for checkpoint in [
        0,
        1,
        observed / 4,
        observed / 2,
        observed * 3 / 4,
        observed - 1,
    ] {
        let calls = Cell::new(0usize);
        let result = capture(&model.bytes).preview_ace16_page_updates(
            "Items",
            "Score",
            &updates,
            &mut budget(),
            &|| {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            },
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Cancelled))),
            "checkpoint {checkpoint}/{observed}"
        );
    }
    assert_eq!(
        preview(&model, &updates).unwrap().candidate_bytes(),
        expected.candidate_bytes()
    );
    assert_eq!(expected.before_bytes(), model.bytes);
}
