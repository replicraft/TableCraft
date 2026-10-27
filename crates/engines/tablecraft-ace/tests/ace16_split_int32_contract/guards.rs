use super::*;

fn source() -> Model {
    database(
        6,
        (1..=500).map(Some).collect(),
        vec![false],
        true,
        false,
        false,
    )
}

fn split(
    model: &Model,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(&model.bytes)
        .preview_ace16_split_int32_updates("Items", "Score", updates, budget, cancelled)
}

#[test]
fn missing_or_nonfree_future_cells_refuse_only_the_required_split() {
    for mode in 0..3 {
        let mut model = source();
        match mode {
            0 => {
                // A complete known inline window for all original pages ends
                // at23. It cannot authorize either new EOF page24/25.
                let payload = vec![0; 8];
                model.bytes[PAGE_BYTES..2 * PAGE_BYTES]
                    .copy_from_slice(&support::data_page(1, &[(0, payload)]));
            }
            1 => {
                let global = map_byte(&model.bytes, 1, ORIGINAL_PAGES);
                model.bytes[global] &= !(1 << (ORIGINAL_PAGES % 8));
            }
            _ => {
                // The old root13 is represented; the future leaves are not.
                let mut payload = vec![0; 8];
                payload[MAP_HEADER + ROOTS[0] / 8] = 1 << (ROOTS[0] % 8);
                let page = INDEX_MAPS[0];
                model.bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES]
                    .copy_from_slice(&support::data_page(0, &[(0, payload)]));
            }
        }
        checked(&model);
        let original = model.bytes.clone();
        let result = split(
            &model,
            &[Int32Update::new(0, 1, i32::MAX)],
            &mut budget(),
            &|| false,
        );
        // Missing representation is outside this bounded allocator. An
        // existing future cell marked nonfree contradicts EOF allocation.
        let expected = if mode == 1 {
            EditError::Inspect(InspectError::Malformed)
        } else {
            EditError::Inspect(InspectError::Unsupported)
        };
        assert_eq!(result.err(), Some(expected), "map guard mode {mode}");
        assert_eq!(model.bytes, original);
        // Neither replacement width nor prefix capacity changes here. Missing
        // future cells must not disable the existing fitting update service.
        let fitting = split(&model, &[Int32Update::new(0, 1, 2)], &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(fitting.candidate_bytes().len(), original.len());
        let mut expected = model.scores.clone();
        expected[0] = Some(2);
        values::verify_values(&model, fitting.candidate_bytes(), &expected);
    }
}

#[test]
fn stale_duplicate_and_constraint_errors_keep_original_image_and_allow_retry() {
    let model = source();
    checked(&model);
    for updates in [
        vec![Int32Update::new(0, 9, i32::MAX)],
        vec![Int32Update::new(0, 1, 2), Int32Update::new(0, 1, 3)],
    ] {
        assert_eq!(
            split(&model, &updates, &mut budget(), &|| false)
                .err()
                .unwrap(),
            EditError::Conflict
        );
    }
    let unique = database(
        6,
        (1..=500).map(Some).collect(),
        vec![false],
        true,
        true,
        false,
    );
    checked(&unique);
    assert!(matches!(
        split(
            &unique,
            &[Int32Update::new(0, 1, 2)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::IndexConstraint(_)))
    ));
    let constrained = database(
        6,
        (1..=500).map(Some).collect(),
        vec![false],
        true,
        false,
        true,
    );
    checked(&constrained);
    assert!(matches!(
        split(
            &constrained,
            &[Int32Update::new(0, 1, i32::MAX)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Constraint(_)))
    ));
    let output = split(
        &constrained,
        &[Int32Update::new(0, 1, i32::MAX - 1)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut expected = constrained.scores.clone();
    expected[0] = Some(i32::MAX - 1);
    let pair = values::verify_split(&constrained, output.candidate_bytes(), &expected, 0);
    values::verify_values(&constrained, output.candidate_bytes(), &expected);
    values::verify_preservation(&constrained, output.candidate_bytes(), 0, &[pair]);
    assert_eq!(output.before_bytes(), constrained.bytes);
}

#[test]
fn exhausted_pools_and_late_cancellation_never_publish_partial_candidate() {
    let model = source();
    checked(&model);
    for mut limited in [
        ResourceBudget::new(0, 1_000_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 0),
    ] {
        assert!(matches!(
            split(
                &model,
                &[Int32Update::new(0, 1, i32::MAX)],
                &mut limited,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let callbacks = Cell::new(0usize);
    let complete = split(
        &model,
        &[Int32Update::new(0, 1, i32::MAX)],
        &mut budget(),
        &|| {
            callbacks.set(callbacks.get() + 1);
            false
        },
    )
    .unwrap();
    let total = callbacks.get();
    assert!(total > 100);
    let progress = Cell::new(0usize);
    let late = || {
        let current = progress.get();
        progress.set(current + 1);
        current >= total - 2
    };
    assert!(matches!(
        split(
            &model,
            &[Int32Update::new(0, 1, i32::MAX)],
            &mut budget(),
            &late
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert!(progress.get() >= total - 1);
    let retry = split(
        &model,
        &[Int32Update::new(0, 1, i32::MAX)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(complete.candidate_bytes(), retry.candidate_bytes());
    assert_eq!(retry.before_bytes(), model.bytes);
}
