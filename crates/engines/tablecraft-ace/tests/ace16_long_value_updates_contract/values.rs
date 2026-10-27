use super::*;

#[test]
fn shared_live_chunks_refuse_even_empty_requests_then_exclusive_retry_succeeds() {
    for code in [5, 6] {
        let shared = source(code, 2);
        let checked = check(&shared);
        assert!(
            checked
                .columns()
                .iter()
                .all(|column| column.status() == LongValueCheckStatus::Checked)
        );
        assert_eq!(checked.columns()[0].shared_references(), 1);
        assert_eq!(checked.columns()[1].shared_references(), 1);
        let original = shared.clone();
        for requests in [vec![], vec![memo_update(Value::Null)]] {
            refused(run(&shared, "Memo", &requests));
        }
        assert_eq!(shared, original);
        let exclusive = source(code, 1);
        check(&exclusive);
        let result = run(&exclusive, "Memo", &[memo_update(Value::Null)]).unwrap();
        assert_eq!(read(result.candidate_bytes())[0][1], Value::Null);
        old_chunks_unchanged(&exclusive, result.candidate_bytes());
    }
}

#[test]
fn cross_column_ownership_overlap_is_readable_but_never_write_authority() {
    let mut bytes = source(6, 1);
    set_maps(&mut bytes, &[MEMO], &[MEMO], &[MEMO]);
    let shared = single(2, MEMO, 0);
    install(
        &mut bytes,
        ROOT,
        &page(10, &[(0, record(1, Some(&shared), Some(&shared)))]),
    );
    let checked = check(&bytes);
    assert_eq!(checked.columns()[0].shared_owned_pages(), 1);
    assert_eq!(checked.columns()[1].shared_owned_pages(), 1);
    assert_eq!(read(&bytes)[0][2], Value::Binary(vec![65, 0]));
    for column in ["Memo", "Ole"] {
        refused(run(&bytes, column, &[]));
    }
    refused(run(&bytes, "Memo", &[memo_update(Value::Null)]));
    // A complete list with no declaration for Ole does not prove that its
    // live external descriptor cannot alias the selected Memo storage.
    support::put16(&mut bytes, 10 * PAGE + DECLARATIONS + 10, u16::MAX);
    let absent = check(&bytes);
    assert_eq!(absent.columns()[1].status(), LongValueCheckStatus::Absent);
    assert_eq!(read(&bytes)[0][2], Value::Binary(vec![65, 0]));
    refused(run(&bytes, "Memo", &[memo_update(Value::Null)]));
    refused(run(&bytes, "Memo", &[]));
    // The corresponding absent-map inline field has no external reference.
    let memo = single(2, MEMO, 0);
    let inline_ole = inline(&[7]);
    install(
        &mut bytes,
        ROOT,
        &page(10, &[(0, record(1, Some(&memo), Some(&inline_ole)))]),
    );
    assert_eq!(
        check(&bytes).columns()[1].status(),
        LongValueCheckStatus::Absent
    );
    let safe = run(&bytes, "Memo", &[memo_update(Value::Null)]).unwrap();
    assert_eq!(read(safe.candidate_bytes())[0][2], Value::Binary(vec![7]));
    for declared in [true, false] {
        let mut catalog_alias = source(6, 1);
        other_table_alias(&mut catalog_alias, declared, true);
        assert_eq!(check(&catalog_alias).columns()[0].shared_owned_pages(), 0);
        let other = read_table(
            &mut Cursor::new(&catalog_alias),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(other.rows()[0][1], Value::Text("A".into()));
        check_table(
            &mut Cursor::new(&catalog_alias),
            CatalogScope::new(1),
            "Other",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        refused(run(&catalog_alias, "Memo", &[memo_update(Value::Null)]));
        refused(run(&catalog_alias, "Memo", &[]));
    }
    let mut inline_other = source(6, 1);
    other_table_alias(&mut inline_other, false, false);
    check(&inline_other);
    assert!(run(&inline_other, "Memo", &[memo_update(Value::Null)]).is_ok());
    let fresh = source(6, 1);
    assert!(run(&fresh, "Memo", &[memo_update(Value::Null)]).is_ok());
}

#[test]
fn orphan_bodies_null_empty_and_semantic_noop_remain_distinct() {
    for code in [5, 6] {
        let bytes = source(code, 1);
        let checked = check(&bytes);
        assert_eq!(checked.columns()[0].owned_rows(), 2);
        assert_eq!(checked.columns()[0].distinct_chunks(), 1);
        assert_eq!(checked.columns()[0].orphan_rows(), 1);
        for value in [
            Value::Null,
            Value::Text(String::new()),
            Value::Text("B".into()),
        ] {
            let result = run(&bytes, "Memo", &[memo_update(value.clone())]).unwrap();
            assert_eq!(read(result.candidate_bytes())[0][1], value);
            assert_eq!(read(result.candidate_bytes())[0][2], Value::Binary(vec![7]));
            old_chunks_unchanged(&bytes, result.candidate_bytes());
            check(result.candidate_bytes());
        }
        let noop = run(&bytes, "Memo", &[memo_update(Value::Text("A".into()))]).unwrap();
        assert_eq!(noop.candidate_bytes(), bytes);
        assert_eq!(noop.changed_rows(), 0);
    }
}

#[test]
fn native_slot_ceiling_uses_slot254_then_allocates_without_slot255() {
    for count in [254, 255] {
        let mut bytes = source(6, 1);
        let mut rows = vec![(0, vec![65, 0]), (0, vec![0xf1, 0xe2, 0xd3])];
        rows.extend((2..count).map(|_| (0, vec![0x7c])));
        install(&mut bytes, MEMO, &lval(&rows));
        let checked = check(&bytes);
        assert_eq!(checked.columns()[0].owned_rows(), count);
        let replacement = Value::Text("Ā".repeat(33));
        let result = run(&bytes, "Memo", &[memo_update(replacement.clone())]).unwrap();
        let after = result.candidate_bytes();
        assert_eq!(read(after)[0][1], replacement);
        old_chunks_unchanged(&bytes, after);
        if count == 254 {
            assert_eq!(after.len(), bytes.len());
            assert_eq!(
                u16::from_le_bytes(
                    after[MEMO * PAGE + 12..MEMO * PAGE + 14]
                        .try_into()
                        .unwrap()
                ),
                255
            );
            assert_eq!(body(after, MEMO, 254), [0, 1].repeat(33));
        } else {
            assert_eq!(after.len(), bytes.len() + PAGE);
            assert_eq!(
                &after[MEMO * PAGE..(MEMO + 1) * PAGE],
                &bytes[MEMO * PAGE..(MEMO + 1) * PAGE]
            );
            assert_eq!(body(after, EOF, 0), [0, 1].repeat(33));
        }
        check(after);
    }
}

#[test]
fn parent_forwarding_and_column_allocation_share_one_exact_eof_ledger() {
    let mut bytes = source(6, 1);
    // Root page has no room to expand either selected descriptor; the hidden
    // filler is not a live logical root or an LVAL chunk.
    let memo = single(2, MEMO, 0);
    let ole = single(1, OLE, 0);
    let original_record = record(1, Some(&memo), Some(&ole));
    let filler = vec![0x9b; 4025];
    install(
        &mut bytes,
        ROOT,
        &page(10, &[(0, original_record), (HIDDEN, filler)]),
    );
    check(&bytes);
    // Inline64 adds64 bytes to the descriptor, forcing ordinary parent EOF
    // forwarding. This does not need or permit a column-owned page allocation.
    let small = Value::Text("Ā".repeat(32));
    let forwarded = run(&bytes, "Memo", &[memo_update(small.clone())]).unwrap();
    let after = forwarded.candidate_bytes();
    assert_eq!(read(after)[0][1], small);
    assert_eq!(word(after, ROOT, 0), word(&bytes, ROOT, 0) | FORWARDED);
    assert_eq!(after.len(), bytes.len() + PAGE);
    let pointer = u32::from_le_bytes(body(after, ROOT, 0)[..4].try_into().unwrap());
    assert_eq!(pointer >> 8, EOF as u32);
    assert_eq!(word(after, EOF, 0) & !OFFSET_MASK, HIDDEN);
    assert_eq!(map_byte(after, 0, EOF) & 1, 1);
    assert_eq!(map_byte(after, 2, EOF) & 1, 0);
    old_chunks_unchanged(&bytes, after);
    // A real fixed-data gap makes the INLINE64 row exceed one ordinary
    // page, while a twelve-byte SINGLE descriptor still fits. Preserve that
    // gap; do not resize a field or alter flags to force admission.
    let mut gap_source = source(6, 1);
    support::put16(&mut gap_source, 10 * PAGE + 84, 4000);
    let mut gap_record = vec![3, 0];
    gap_record.extend(vec![0xa6; 4000]);
    gap_record.extend(1_i32.to_le_bytes());
    for _ in 0..3 {
        gap_record.extend(4006_u16.to_le_bytes());
    }
    gap_record.extend(2_u16.to_le_bytes());
    gap_record.push(1);
    assert_eq!(gap_record.len(), 4015);
    install(&mut gap_source, ROOT, &page(10, &[(0, gap_record)]));
    check(&gap_source);
    assert_eq!(
        read(&gap_source)[0],
        vec![Value::Int32(1), Value::Null, Value::Null]
    );
    let fallback_value = Value::Text("Ā".repeat(32));
    let fallback_request = SlotScalarUpdate::new(0, Value::Null, fallback_value.clone());
    let fallback = run(&gap_source, "Memo", &[fallback_request]).unwrap();
    let fallback_bytes = fallback.candidate_bytes();
    assert_eq!(read(fallback_bytes)[0][1], fallback_value);
    assert_eq!(body(fallback_bytes, MEMO, 2), [0, 1].repeat(32));
    let report = check(fallback_bytes);
    assert_eq!(report.columns()[0].values().single(), 1);
    assert_eq!(report.columns()[0].values().inline(), 0);
    assert_eq!(fallback_bytes.len(), gap_source.len() + PAGE);
    let forwarded = body(fallback_bytes, ROOT, 0);
    let pointer = u32::from_le_bytes(forwarded[..4].try_into().unwrap());
    assert_eq!(pointer >> 8, EOF as u32);
    let target = body(fallback_bytes, EOF, 0);
    assert_eq!(target.len(), 4027);
    assert_eq!(&target[2..4002], &[0xa6; 4000]);
    assert_eq!(&target[4006..4018], single(64, MEMO, 2));
    old_chunks_unchanged(&gap_source, fallback_bytes);
    // A NULL source has no descriptor storage. External encoding adds12 bytes
    // to the parent and needs a fourth EOF page after three LVAL pages.
    let mut null_source = source(6, 1);
    let ole = single(1, OLE, 0);
    install(
        &mut null_source,
        ROOT,
        &page(
            10,
            &[(0, record(1, None, Some(&ole))), (HIDDEN, vec![0x9b; 4040])],
        ),
    );
    assert_eq!(body(&null_source, ROOT, 0).len(), 27);
    check(&null_source);
    let large = Value::Text("Ā".repeat(4073));
    let request = SlotScalarUpdate::new(0, Value::Null, large.clone());
    let chained = run(&null_source, "Memo", &[request]).unwrap();
    let result = chained.candidate_bytes();
    assert_eq!(read(result)[0][1], large);
    old_chunks_unchanged(&null_source, result);
    let expected = combined_expected(&null_source);
    assert_eq!(result, expected);
    let checked = check(result);
    assert_eq!(checked.columns()[0].values().chained(), 1);
    assert_eq!(checked.columns()[0].distinct_chunks(), 3);
    assert_eq!(checked.columns()[0].orphan_rows(), 2);
    assert_eq!(body(result, ROOT, 1), body(&null_source, ROOT, 1));
}
