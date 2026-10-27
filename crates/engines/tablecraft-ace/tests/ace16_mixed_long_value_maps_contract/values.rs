use super::*;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn request(value: Value) -> SlotScalarUpdate {
    SlotScalarUpdate::new(0, Value::Text("A".into()), value)
}

fn run(bytes: &[u8], value: Value) -> Result<Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_long_value_updates(
        "Items",
        "Memo",
        &[request(value)],
        &mut budget(),
        &|| false,
    )
}

fn full_candidate() -> Vec<(u16, Vec<u8>)> {
    vec![
        (0, vec![65, 0]),
        (0, vec![0xf1, 0xe2, 0xd3]),
        (0, vec![0x9b; 4069]),
    ]
}

#[test]
fn fitted_chunk_append_preserves_indirect_pointer_prefix_and_all_map_bytes() {
    for code in [5, 6] {
        let before = source(code, true, BITMAP);
        assert_eq!(
            checked(&before).columns()[0].status(),
            LongValueCheckStatus::Checked
        );
        let value = Value::Text("Ā".repeat(33));
        let after = run(&before, value.clone()).unwrap();
        let mut expected = before.clone();
        install(
            &mut expected,
            MEMO,
            &lval(&[
                (0, vec![65, 0]),
                (0, vec![0xf1, 0xe2, 0xd3]),
                (0, [0, 1].repeat(33)),
            ]),
        );
        select_descriptor(&mut expected, &single(66, MEMO, 2));
        assert_eq!(after.candidate_bytes(), expected);
        assert_eq!(read(&expected)[0][1], value);
        assert_eq!(checked(&expected).columns()[0].owned_rows(), 3);
        let noop = run(&before, Value::Text("A".into())).unwrap();
        assert_eq!(noop.candidate_bytes(), before);
        let empty = capture(&before)
            .preview_ace16_long_value_updates("Items", "Memo", &[], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.candidate_bytes(), before);
    }
}

#[test]
fn eof_single_and_chain_compose_exact_existing_bitmap_hint_and_global_cells() {
    for chained in [false, true] {
        let mut before = source(6, true, BITMAP);
        install(&mut before, MEMO, &lval(&full_candidate()));
        checked(&before);
        let units = if chained { 4073 } else { 33 };
        let value = Value::Text("Ā".repeat(units));
        let after = run(&before, value.clone()).unwrap();
        let mut expected = before.clone();
        let pages = if chained { 3 } else { 1 };
        expected.resize((EOF + pages) * PAGE, 0);
        if chained {
            let sizes = [4072, 4072, 2];
            for (position, size) in sizes.into_iter().enumerate() {
                let next = if position == 2 {
                    0
                } else {
                    ((EOF + position + 1) as u32) << 8
                };
                let mut body = next.to_le_bytes().to_vec();
                body.extend([0, 1].repeat(size / 2));
                install(&mut expected, EOF + position, &lval(&[(0, body)]));
            }
            select_descriptor(&mut expected, &descriptor(0, 8146, (EOF as u32) << 8));
        } else {
            install(&mut expected, EOF, &lval(&[(0, [0, 1].repeat(33))]));
            select_descriptor(&mut expected, &single(66, EOF, 0));
        }
        set_inline(&mut expected, 11, 3, MEMO, false);
        for position in 0..pages {
            set_owned(&mut expected, EOF + position, true);
            set_inline(&mut expected, 1, 0, EOF + position, false);
        }
        set_inline(&mut expected, 11, 3, EOF + pages - 1, true);
        assert_eq!(after.candidate_bytes(), expected);
        assert_eq!(read(&expected)[0][1], value);
        assert_eq!(checked(&expected).columns()[0].owned_pages(), 1 + pages);
        assert_eq!(&expected[BITMAP * PAGE..BITMAP * PAGE + 4], &[5, 1, 0, 0]);
    }
}

#[test]
fn absent_represented_owned_or_hint_eof_cells_never_grow_map_storage() {
    let before = source(6, false, 0);
    assert_eq!(
        checked(&before).columns()[0].status(),
        LongValueCheckStatus::Checked
    );
    assert_eq!(
        run(&before, Value::Text("Ā".repeat(33))).err(),
        Some(EditError::Inspect(InspectError::Unsupported))
    );
    let mut before = source(6, true, BITMAP);
    install(&mut before, MEMO, &lval(&full_candidate()));
    let rows = vec![
        (0, inline_map(&[ROOT])),
        (0, inline_map(&[ROOT])),
        (0, indirect(BITMAP)),
        (0, vec![0, 0, 0, 0, 0, 0, 0x20, 0]),
        (0, inline_map(&[OLE])),
        (0, inline_map(&[OLE])),
    ];
    install(&mut before, 11, &page(0, &rows));
    checked(&before);
    assert_eq!(
        run(&before, Value::Text("Ā".repeat(33))).err(),
        Some(EditError::Inspect(InspectError::Unsupported))
    );

    let mut unsupported = source(6, true, BITMAP);
    install(&mut unsupported, 20, &bitmap(&[MEMO]));
    let rows = vec![
        (0, inline_map(&[ROOT])),
        (0, inline_map(&[ROOT])),
        (0, indirect(BITMAP)),
        (0, indirect(20)),
        (0, inline_map(&[OLE])),
        (0, inline_map(&[OLE])),
    ];
    install(&mut unsupported, 11, &page(0, &rows));
    assert_eq!(
        checked(&unsupported).columns()[0].status(),
        LongValueCheckStatus::IndirectMaps
    );
    for updates in [
        vec![],
        vec![request(Value::Text("A".into()))],
        vec![request(Value::Text("Ā".repeat(33)))],
    ] {
        assert_eq!(
            capture(&unsupported)
                .preview_ace16_long_value_updates("Items", "Memo", &updates, &mut budget(), &|| {
                    false
                },)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

#[test]
fn empty_bitmap_storage_alias_refuses_while_disjoint_shared_containers_work() {
    for alias in [false, true] {
        let mut before = source(6, false, BITMAP);
        old_model::other_table_alias(&mut before, false, false);
        let other_bitmap = if alias { BITMAP } else { 20 };
        install(&mut before, 20, &bitmap(&[]));
        let rows = vec![
            (0, inline_map(&[17])),
            (0, indirect(other_bitmap)),
            (0, inline_map(&[])),
            (0, inline_map(&[])),
        ];
        install(&mut before, 16, &page(0, &rows));
        checked(&before);
        check_table(
            &mut Cursor::new(&before),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if alias {
            assert_eq!(
                run(&before, Value::Text("Ā".repeat(33))).err(),
                Some(EditError::Unsupported(WriteBlocker::RowLayout))
            );
            assert_eq!(
                capture(&before)
                    .preview_ace16_long_value_updates("Items", "Memo", &[], &mut budget(), &|| {
                        false
                    })
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::RowLayout))
            );
        } else {
            let after = run(&before, Value::Text("Ā".repeat(33))).unwrap();
            assert_eq!(
                &after.candidate_bytes()[15 * PAGE..18 * PAGE],
                &before[15 * PAGE..18 * PAGE]
            );
            assert_eq!(
                &after.candidate_bytes()[20 * PAGE..21 * PAGE],
                &before[20 * PAGE..21 * PAGE]
            );
        }
    }
}

#[test]
fn mixed_maps_keep_resource_cancellation_and_exact_retry_boundaries() {
    let mut before = source(6, true, BITMAP);
    install(&mut before, MEMO, &lval(&full_candidate()));
    checked(&before);
    for limits in [
        (1, 100_000, 50_000_000),
        (128 * 1024 * 1024, 1, 50_000_000),
        (128 * 1024 * 1024, 100_000, 1),
    ] {
        assert!(
            capture(&before)
                .preview_ace16_long_value_updates(
                    "Items",
                    "Memo",
                    &[request(Value::Text("Ā".repeat(33)))],
                    &mut ResourceBudget::new(limits.0, limits.1, limits.2),
                    &|| false,
                )
                .is_err()
        );
    }
    let calls = Cell::new(0);
    assert!(
        capture(&before)
            .preview_ace16_long_value_updates(
                "Items",
                "Memo",
                &[request(Value::Text("Ā".repeat(33)))],
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == 200
                },
            )
            .is_err()
    );
    assert_eq!(calls.get(), 200);
    let first = run(&before, Value::Text("Ā".repeat(33))).unwrap();
    let retry = run(&before, Value::Text("Ā".repeat(33))).unwrap();
    assert_eq!(first.candidate_bytes(), retry.candidate_bytes());
}
