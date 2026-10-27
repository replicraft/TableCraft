use super::*;

const ROOT_POINTER_AFTER_USAGE: usize = 4;
const GLOBAL_POINTER: usize = 1;
const MAP_HEADER: usize = 5;

#[test]
fn two_promoted_physical_owners_preserve_full_images_aliases_and_shared_global_byte() {
    for code in [5, 6] {
        let model = model::source(code);
        model::assert_literal_source(&model);
        before(&model, 4);
        let original = model.bytes.clone();
        verify(&model, &run(&model).unwrap());
        assert_eq!(model.bytes, original);
        let noop = capture(&model.bytes)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &[request(0)],
                &mut fixed_model::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(noop.changed_rows(), 0);
        assert_eq!(noop.candidate_bytes(), original);
    }
}

#[test]
fn selected_and_foreign_map_rows_can_share_a_container_when_their_ranges_are_disjoint() {
    for code in [5, 6] {
        let mut model = model::source(code);
        model::share_container_distinct_rows(&mut model);
        before(&model, 4);
        let original = model.bytes.clone();
        verify(&model, &run(&model).unwrap());
        assert_eq!(model.bytes, original);
    }
}

#[test]
fn promotion_refuses_shiftable_inline_windows_missing_cells_and_every_nonfree_append_role() {
    let mut shifted = model::source(6);
    for (physical, new_root) in [100, 106].into_iter().enumerate() {
        let old_root = model::ROOTS[physical];
        let source = shifted.bytes[old_root * model::PAGE..(old_root + 1) * model::PAGE].to_vec();
        model::install(&mut shifted.bytes, new_root, &source);
        let pointer =
            model::selected_usage_position(&shifted.bytes, physical) + ROOT_POINTER_AFTER_USAGE;
        support::put32(&mut shifted.bytes, pointer, new_root as u32);
        model::install(
            &mut shifted.bytes,
            model::MAPS[physical],
            &support::data_page(0, &[(0, model::map(&[new_root]))]),
        );
        // All original and next child members fit within512 addresses after a
        // shift. This profile must refuse rather than allocate an unneeded map.
        assert!(model::EOF - new_root < (model::MAP_BYTES - MAP_HEADER) * 8);
    }
    before(&shifted, 4);
    assert_eq!(
        run(&shifted).err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );

    let mut missing = model::source(6);
    let (global_start, _) = model::row_range(&missing.bytes, 1, 0);
    support::put32(&mut missing.bytes, global_start + GLOBAL_POINTER, 0);
    before(&missing, 4);
    assert_eq!(
        run(&missing).err(),
        Some(EditError::Inspect(InspectError::Unsupported))
    );

    for page in model::EOF..model::EOF + 6 {
        let mut nonfree = model::source(6);
        let at = model::GLOBAL_BITMAP * model::PAGE + model::BITMAP_HEADER + page / 8;
        nonfree.bytes[at] &= !(1 << (page % 8));
        before(&nonfree, 4);
        let original = nonfree.bytes.clone();
        assert_eq!(
            run(&nonfree).err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
        assert_eq!(nonfree.bytes, original);
    }
    let retry = model::source(6);
    verify(&retry, &run(&retry).unwrap());
}

#[test]
fn immutable_requests_and_all_resource_pools_cancel_atomically_then_retry_exactly() {
    let model = model::source(6);
    before(&model, 4);
    for requests in [
        vec![FixedScalarUpdate::new(
            0,
            Value::Int32(1),
            Value::Int32(i32::MAX),
        )],
        vec![request(i32::MAX), request(i32::MAX)],
        vec![FixedScalarUpdate::new(
            model::ROWS,
            Value::Int32(0),
            Value::Int32(i32::MAX),
        )],
    ] {
        assert_eq!(
            capture(&model.bytes)
                .preview_ace16_split_fixed_updates(
                    "Items",
                    "Score",
                    &requests,
                    &mut fixed_model::budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Conflict),
        );
    }
    for limits in [
        (1, 1_000_000, 100_000_000),
        (256 * 1024 * 1024, 1, 100_000_000),
        (256 * 1024 * 1024, 1_000_000, 1),
    ] {
        assert!(matches!(
            capture(&model.bytes).preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &[request(i32::MAX)],
                &mut ResourceBudget::new(limits.0, limits.1, limits.2),
                &|| false,
            ),
            Err(EditError::Inspect(InspectError::Limit(_))),
        ));
    }
    let measured = Cell::new(0);
    let accepted = capture(&model.bytes)
        .preview_ace16_split_fixed_updates(
            "Items",
            "Score",
            &[request(i32::MAX)],
            &mut fixed_model::budget(),
            &|| {
                measured.set(measured.get() + 1);
                false
            },
        )
        .unwrap();
    verify(&model, &accepted);
    assert!(measured.get() > 1000);
    let threshold = measured.get() - 1;
    let calls = Cell::new(0);
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_split_fixed_updates(
                "Items",
                "Score",
                &[request(i32::MAX)],
                &mut fixed_model::budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == threshold
                },
            )
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled)),
    );
    assert_eq!(calls.get(), threshold);
    verify(&model, &run(&model).unwrap());
}
