use super::*;

#[test]
fn parent_capacity_overflow_grows_the_retained_root_after_a_healthy_before_image() {
    let mut scores = (1..=600)
        .map(|offset| Some(i32::MIN + offset))
        .collect::<Vec<_>>();
    scores.extend((0..301).map(|position| {
        Some(if position == 0 {
            -1
        } else {
            (position as i64 * i32::MAX as i64 / 301) as i32
        })
    }));
    let mut counts = vec![600];
    counts.extend(vec![1; 301]);
    let model = source(6, scores, counts, vec![false], false);
    check(&model);
    let root = ROOTS[0] * PAGE;
    assert_eq!(decoded(&model.bytes, ROOTS[0]).len(), 301);
    assert_eq!(root_model::get16(&model.bytes, root + PREFIX), 1);
    assert_eq!(root_model::get16(&model.bytes, root + FREE), 3);
    // One more full13-byte key/ROW/child bound, with only one common key byte,
    // needs3625 bytes. Recompressing cannot fit it in3616, so the root grows.
    assert_eq!(302 * 13 - 301, 3625);
    let original = model.bytes.clone();
    let result = run(&model, Int32Update::new(0, i32::MIN + 1, i32::MAX)).unwrap();
    assert_eq!(result.candidate_bytes().len(), original.len() + 3 * PAGE);
    assert_eq!(result.candidate_bytes()[root + HEIGHT], 2);
    assert_eq!(decoded(result.candidate_bytes(), ROOTS[0]).len(), 2);
    assert_eq!(root_model::get32(result.candidate_bytes(), root + TAIL), 0);
    verify_values(&model, result.candidate_bytes(), 0, i32::MAX);
    check_table(
        &mut Cursor::new(result.candidate_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(model.bytes, original);
    let noop = run(&model, Int32Update::new(0, i32::MIN + 1, i32::MIN + 1)).unwrap();
    assert_eq!(noop.candidate_bytes(), original);
}

#[test]
fn future_map_failures_stale_requests_and_late_cancel_keep_the_source_unchanged() {
    let original_model = source(6, groups(false), vec![600; 3], vec![false], false);
    check(&original_model);
    for mode in 0..3 {
        let mut model = source(6, groups(false), vec![600; 3], vec![false], false);
        match mode {
            0 => {
                let payload = vec![0; MAP_HEADER + model.eof / 8];
                model.bytes[PAGE..2 * PAGE]
                    .copy_from_slice(&support::data_page(1, &[(0, payload)]));
            }
            1 => {
                let at = root_model::map_byte(&model.bytes, 1, model.eof);
                model.bytes[at] &= !(1 << (model.eof % 8));
            }
            _ => {
                let mut payload = map(&[ROOTS[0], 32, 33, 34]);
                payload.truncate(MAP_HEADER + model.eof / 8);
                let page = MAPS[0];
                model.bytes[page * PAGE..(page + 1) * PAGE]
                    .copy_from_slice(&support::data_page(0, &[(0, payload)]));
            }
        }
        check(&model);
        let original = model.bytes.clone();
        let error = run(
            &model,
            Int32Update::new(600, model.scores[600].unwrap(), 0x01ff_ffff),
        )
        .err();
        assert_eq!(
            error,
            Some(EditError::Inspect(if mode == 1 {
                InspectError::Malformed
            } else {
                InspectError::Unsupported
            }))
        );
        assert_eq!(model.bytes, original);
        let fitting = run(&model, Int32Update::new(1, 2, 3)).unwrap();
        assert_eq!(fitting.candidate_bytes().len(), original.len());
    }
    let model = original_model;
    assert_eq!(
        run(&model, Int32Update::new(600, 1, 0x01ff_ffff)).err(),
        Some(EditError::Conflict)
    );
    let duplicate = [
        Int32Update::new(600, model.scores[600].unwrap(), 0x01ff_ffff),
        Int32Update::new(600, model.scores[600].unwrap(), 0x01ff_ffff),
    ];
    let duplicate = root_model::capture(&model.bytes).preview_ace16_split_int32_updates(
        "Items",
        "Score",
        &duplicate,
        &mut budget(),
        &|| false,
    );
    assert_eq!(duplicate.err(), Some(EditError::Conflict));
    let update = [Int32Update::new(
        600,
        model.scores[600].unwrap(),
        0x01ff_ffff,
    )];
    for mut limited in [
        ResourceBudget::new(0, 1_000_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 0),
    ] {
        let result = root_model::capture(&model.bytes).preview_ace16_split_int32_updates(
            "Items",
            "Score",
            &update,
            &mut limited,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0);
    let complete = root_model::capture(&model.bytes)
        .preview_ace16_split_int32_updates("Items", "Score", &update, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let count = calls.get();
    assert!(count > 3);
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
    let retry = run(
        &model,
        Int32Update::new(600, model.scores[600].unwrap(), 0x01ff_ffff),
    )
    .unwrap();
    assert_eq!(retry.candidate_bytes(), complete.candidate_bytes());
}
