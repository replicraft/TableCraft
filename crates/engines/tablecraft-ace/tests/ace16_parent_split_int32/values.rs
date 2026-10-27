use super::*;

#[test]
fn parent_overflow_keeps_root_and_allocates_leaf_then_right_and_left_nodes() {
    for code in [5, 6] {
        let model = parent_source(code);
        check(&model);
        assert_eq!(decoded(&model.bytes, ROOTS[0]).len(), 301);
        assert_eq!(root_model::get16(&model.bytes, ROOTS[0] * PAGE + FREE), 3);
        let expected = parent_expected(&model);
        let result = run(&model, Int32Update::new(0, i32::MIN + 1, i32::MAX)).unwrap();
        assert_eq!(result.candidate_bytes(), expected);
        verify_values(&model, result.candidate_bytes(), 0, i32::MAX);
        let original = model.bytes.clone();
        let noop = run(&model, Int32Update::new(0, i32::MIN + 1, i32::MIN + 1)).unwrap();
        assert_eq!(noop.candidate_bytes(), original);
    }
}

#[test]
fn parent_growth_limits_late_cancel_and_retry_remain_atomic() {
    let model = parent_source(6);
    check(&model);
    let update = [Int32Update::new(0, i32::MIN + 1, i32::MAX)];
    for mut limits in [
        ResourceBudget::new(0, 1_000_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 0),
    ] {
        let result = root_model::capture(&model.bytes).preview_ace16_split_int32_updates(
            "Items",
            "Score",
            &update,
            &mut limits,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0);
    let result = root_model::capture(&model.bytes)
        .preview_ace16_split_int32_updates("Items", "Score", &update, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let count = calls.get();
    let calls = Cell::new(0);
    let cancelled = root_model::capture(&model.bytes).preview_ace16_split_int32_updates(
        "Items",
        "Score",
        &update,
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            calls.get() >= count - 1
        },
    );
    assert_eq!(
        cancelled.err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    let retry = run(&model, Int32Update::new(0, i32::MIN + 1, i32::MAX)).unwrap();
    assert_eq!(retry.candidate_bytes(), result.candidate_bytes());
    assert_eq!(result.candidate_bytes(), parent_expected(&model));
}
