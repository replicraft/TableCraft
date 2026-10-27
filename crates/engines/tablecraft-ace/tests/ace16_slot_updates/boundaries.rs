use super::*;

#[test]
fn stale_duplicate_growth_and_wrong_carriers_refuse_atomically() {
    // Valid read/DELETE flag-zero fixtures do not establish slot-write
    // authority. Both new variable-field admissions require observed flag2.
    for original in [
        encoded_model::text(1, false, false),
        encoded_model::binary(false, false),
    ] {
        check_table(
            &mut Cursor::new(&original.bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            preview(&original, &[]),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        assert!(matches!(
            preview(
                &original,
                &[SlotScalarUpdate::new(0, Value::Null, Value::Null)]
            ),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }

    let model = model::text(1, false, false);
    assert!(matches!(
        preview(
            &model,
            &[SlotScalarUpdate::new(
                2,
                Value::Text("A".into()),
                Value::Text("é".into())
            )]
        ),
        Err(EditError::Conflict)
    ));
    assert!(matches!(
        preview(
            &model,
            &[SlotScalarUpdate::new(
                2,
                Value::Text("a".into()),
                Value::Text("longer".into())
            )]
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    assert!(matches!(
        preview(
            &model,
            &[SlotScalarUpdate::new(
                2,
                Value::Text("a".into()),
                Value::Binary(vec![1, 2])
            )]
        ),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    assert!(matches!(
        preview(
            &model,
            &[
                SlotScalarUpdate::new(2, Value::Text("a".into()), Value::Text("é".into())),
                SlotScalarUpdate::new(2, Value::Text("a".into()), Value::Text("é".into()))
            ]
        ),
        Err(EditError::Conflict)
    ));
    let output = preview(
        &model,
        &[SlotScalarUpdate::new(
            2,
            Value::Text("a".into()),
            Value::Text("a".into()),
        )],
    )
    .unwrap();
    assert_eq!(output.changed_rows(), 0);
    assert_eq!(output.candidate_bytes(), model.bytes);
}

#[test]
fn typed_limits_cancellation_reuse_and_existing_binary_update_gate_remain() {
    use std::cell::Cell;
    use tablecraft_ace::InspectError;
    use tablecraft_core::limits::ResourceBudget;

    let model = model::binary(false, true);
    let updates = [SlotScalarUpdate::new(
        4,
        Value::Binary((1..=8).collect()),
        Value::Binary(vec![255; 8]),
    )];
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            capture(&model.bytes).preview_ace16_slot_updates(
                "Items",
                "Score",
                &updates,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0usize);
    let expected = capture(&model.bytes)
        .preview_ace16_slot_updates("Items", "Score", &updates, &mut budget(), &|| {
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
        let result = capture(&model.bytes).preview_ace16_slot_updates(
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
    assert!(matches!(
        capture(&model.bytes).preview_ace16_fixed_updates(
            "Items",
            "Id",
            &[],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));
}
