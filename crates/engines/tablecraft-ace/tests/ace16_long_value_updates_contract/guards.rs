use super::*;

#[test]
fn metadata_and_allocation_guards_remain_closed_for_empty_requests() {
    for mode in 0..5 {
        let mut bytes = source(6, 1);
        match mode {
            0 => bytes[10 * PAGE + MEMO_FLAGS] |= 0x80,
            1 => bytes[10 * PAGE + MEMO_EXTENDED_FLAGS] = 1,
            2 => bytes[10 * PAGE + MEMO_EXTENDED_FLAGS] = 0xc0,
            3 => selected_property(&mut bytes, "Caption"),
            _ => support::put32(&mut bytes, 10 * PAGE + DECLARATIONS + 2, 0),
        }
        // Calculated metadata changes the normal scalar admission. Every other
        // source remains independently readable, including unknown map leads.
        if mode != 2 {
            assert_eq!(read(&bytes)[0][1], Value::Text("A".into()));
        }
        refused(run(&bytes, "Memo", &[]));
        refused(run(&bytes, "Memo", &[memo_update(Value::Null)]));
    }
    let mut malformed = source(6, 1);
    malformed[MEMO * PAGE + 4..MEMO * PAGE + 8].copy_from_slice(b"BAD!");
    assert!(
        check_table_long_values(
            &mut Cursor::new(&malformed),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(run(&malformed, "Memo", &[]).is_err());
    let mut nonfree = source(6, 1);
    let map_start = usize::from(word(&nonfree, 1, 0) & OFFSET_MASK);
    nonfree[PAGE + map_start + MAP_HEADER + MEMO / 8] |= 1 << (MEMO % 8);
    assert!(
        check_table_long_values(
            &mut Cursor::new(&nonfree),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert!(run(&nonfree, "Memo", &[]).is_err());
}

#[test]
fn stale_duplicate_missing_and_bad_carriers_never_publish_a_prefix() {
    let bytes = source(6, 1);
    check(&bytes);
    for requests in [
        vec![SlotScalarUpdate::new(
            0,
            Value::Text("wrong".into()),
            Value::Null,
        )],
        vec![
            memo_update(Value::Null),
            memo_update(Value::Text("B".into())),
        ],
        vec![SlotScalarUpdate::new(
            1,
            Value::Text("A".into()),
            Value::Null,
        )],
    ] {
        assert_eq!(
            run(&bytes, "Memo", &requests).err(),
            Some(EditError::Conflict)
        );
    }
    refused(run(
        &bytes,
        "Memo",
        &[memo_update(Value::Binary(vec![1, 2]))],
    ));
    let wrong_ole = SlotScalarUpdate::new(0, Value::Binary(vec![7]), Value::Text("B".into()));
    refused(run(&bytes, "Ole", &[wrong_ole]));
    let mut aliased = source(6, 2);
    let pointer = ((ROOT as u32) << 8 | 2).to_le_bytes().to_vec();
    let memo = inline(&[65, 0]);
    let ole = inline(&[7]);
    install(
        &mut aliased,
        ROOT,
        &page(
            10,
            &[
                (FORWARDED, pointer.clone()),
                (FORWARDED, pointer),
                (HIDDEN, record(1, Some(&memo), Some(&ole))),
            ],
        ),
    );
    let alias_check = check(&aliased);
    assert_eq!(alias_check.rows(), 2);
    assert_eq!(alias_check.columns()[0].shared_references(), 0);
    assert_eq!(read(&aliased).len(), 2);
    refused(run(&aliased, "Memo", &[memo_update(Value::Null)]));
    let mut constrained = source(6, 1);
    table_rule(&mut constrained, "Len([Memo]) <= 1");
    check(&constrained);
    let rejected = run(
        &constrained,
        "Memo",
        &[memo_update(Value::Text("AB".into()))],
    );
    assert_eq!(
        rejected.err(),
        Some(EditError::Inspect(InspectError::Constraint(
            tablecraft_ace::ConstraintViolation::TableRule
        )))
    );
    let accepted = run(
        &constrained,
        "Memo",
        &[memo_update(Value::Text("B".into()))],
    )
    .unwrap();
    assert_eq!(
        read(accepted.candidate_bytes())[0][1],
        Value::Text("B".into())
    );
    let good = run(&bytes, "Memo", &[memo_update(Value::Null)]).unwrap();
    assert_eq!(good.before_bytes(), bytes);
    old_chunks_unchanged(&bytes, good.candidate_bytes());
}

#[test]
fn paid_inventory_encoding_and_final_validation_cancel_atomically_then_retry() {
    let bytes = source(6, 1);
    check(&bytes);
    let updates = [memo_update(Value::Text("Ā".repeat(4073)))];
    for mut limited in [
        ResourceBudget::new(0, 100_000, 50_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 50_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        let result = capture(&bytes).preview_ace16_long_value_updates(
            "Items",
            "Memo",
            &updates,
            &mut limited,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0);
    let complete = capture(&bytes)
        .preview_ace16_long_value_updates("Items", "Memo", &updates, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let baseline = calls.get();
    assert!(baseline > 3);
    let calls = Cell::new(0);
    let cancelled = capture(&bytes).preview_ace16_long_value_updates(
        "Items",
        "Memo",
        &updates,
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            calls.get() >= baseline - 1
        },
    );
    assert_eq!(
        cancelled.err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    let retry = run(&bytes, "Memo", &updates).unwrap();
    assert_eq!(retry.candidate_bytes(), complete.candidate_bytes());
    old_chunks_unchanged(&bytes, retry.candidate_bytes());
}
