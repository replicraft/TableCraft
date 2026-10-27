use super::*;

#[test]
fn mixed_maps_capture_complete_owned_orphan_and_shared_reference_counts() {
    for code in [5, 6] {
        let mut bytes = source(code, true, BITMAP);
        assert_eq!(read(&bytes)[0][1], Value::Text("A".into()));
        let report = checked(&bytes);
        let memo = &report.columns()[0];
        assert_eq!(memo.status(), LongValueCheckStatus::Checked);
        assert_eq!(memo.owned_pages(), 1);
        assert_eq!(memo.candidate_pages(), 1);
        assert_eq!(memo.owned_rows(), 2);
        assert_eq!(memo.distinct_chunks(), 1);
        assert_eq!(memo.orphan_rows(), 1);
        let descriptor = single(2, MEMO, 0);
        let ole = single(1, OLE, 0);
        support::put32(&mut bytes, 10 * PAGE + 16, 2);
        install(
            &mut bytes,
            ROOT,
            &page(
                10,
                &[
                    (0, record(1, &descriptor, &ole)),
                    (0, record(2, &descriptor, &ole)),
                ],
            ),
        );
        let shared = checked(&bytes);
        assert_eq!(shared.columns()[0].status(), LongValueCheckStatus::Checked);
        assert_eq!(shared.columns()[0].shared_references(), 1);
        assert_eq!(read(&bytes).len(), 2);
    }
}

#[test]
fn source_bitmap_signature_allocation_and_live_membership_are_checked() {
    for mode in 0..4 {
        let mut bytes = source(6, true, BITMAP);
        checked(&bytes);
        match mode {
            0 => bytes[BITMAP * PAGE] = 0,
            1 => set_inline(&mut bytes, 1, 0, BITMAP, true),
            2 => set_owned(&mut bytes, MEMO, false),
            _ => {
                // A complete empty candidate map isolates missing live-owned
                // membership rather than a candidate-subset error.
                let rows = vec![
                    (0, inline_map(&[ROOT])),
                    (0, inline_map(&[ROOT])),
                    (0, indirect(0)),
                    (0, inline_map(&[])),
                    (0, inline_map(&[OLE])),
                    (0, inline_map(&[OLE])),
                ];
                install(&mut bytes, 11, &page(0, &rows));
            }
        }
        assert_eq!(read(&bytes)[0][1], Value::Text("A".into()));
        assert_eq!(
            check_table_long_values(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(InspectError::Malformed)
        );
    }
}

#[test]
fn empty_owned_pointer_can_be_complete_but_indirect_candidates_remain_explicit() {
    let bytes = source(6, false, 0);
    let report = checked(&bytes);
    assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Checked);
    assert_eq!(report.columns()[0].owned_pages(), 0);
    assert_eq!(report.columns()[0].distinct_chunks(), 0);
    assert_eq!(read(&bytes)[0][1], Value::Text("A".into()));
    let mut bytes = source(6, true, BITMAP);
    let rows = vec![
        (0, inline_map(&[ROOT])),
        (0, inline_map(&[ROOT])),
        (0, indirect(BITMAP)),
        (0, indirect(BITMAP)),
        (0, inline_map(&[OLE])),
        (0, inline_map(&[OLE])),
    ];
    install(&mut bytes, 11, &page(0, &rows));
    let report = checked(&bytes);
    assert_eq!(
        report.columns()[0].status(),
        LongValueCheckStatus::IndirectMaps
    );
    assert_eq!(read(&bytes)[0][1], Value::Text("A".into()));
}
