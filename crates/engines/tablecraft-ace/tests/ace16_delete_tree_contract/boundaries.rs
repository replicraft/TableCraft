use super::{model::*, support};
use std::cell::Cell;
use tablecraft_ace::{EditError, InspectError, RowDelete};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn batch_retirement_without_global_allocation_metadata_refuses_atomically() {
    for bounded in [false, true] {
        for deep in [false, true] {
            let model = database(6, false, bounded, deep, false);
            let rows = model.leaves[0].1.clone();
            for &row in &rows {
                assert!(preview(&model.bytes, &[request(&model, row)]).is_ok());
            }
            let requests = rows
                .iter()
                .map(|&row| request(&model, row))
                .collect::<Vec<_>>();
            assert!(matches!(
                preview(&model.bytes, &requests),
                Err(EditError::Inspect(InspectError::Malformed))
            ));
            let all = (0..8).map(|row| request(&model, row)).collect::<Vec<_>>();
            assert!(matches!(
                preview(&model.bytes, &all),
                Err(EditError::Inspect(InspectError::Malformed))
            ));
            let retry = preview(&model.bytes, &[request(&model, rows[0])]).unwrap();
            verify(&model, &retry, &[rows[0]]);
        }
    }
}

#[test]
fn malformed_node_links_height_ownership_map_and_separator_fail_even_on_no_op() {
    let model = database(6, false, true, true, false);
    let original = &model.bytes;
    let mut cases = Vec::new();

    let mut owner = original.clone();
    support::put32(&mut owner, 17 * 4096 + 4, 11);
    cases.push(owner);

    let mut cycle = original.clone();
    // Root's first bound is a non-NULL key (5 bytes) plus row ref (4),
    // followed by the child page number (4).
    cycle[13 * 4096 + 489..13 * 4096 + 493].copy_from_slice(&13_u32.to_be_bytes());
    cases.push(cycle);

    let mut stamp = original.clone();
    stamp[17 * 4096 + 8] = 1;
    cases.push(stamp);

    let mut height = original.clone();
    height[17 * 4096 + 26] = 2;
    cases.push(height);

    let mut links = original.clone();
    support::put32(&mut links, 16 * 4096 + 12, 19);
    cases.push(links);

    let mut map = original.clone();
    // Map row is eight bytes at the end of its ordinary data page.
    map[15 * 4096 - 2] &= !0x80;
    cases.push(map);

    let mut separator = original.clone();
    separator[17 * 4096 + 484] = 0;
    cases.push(separator);

    for (case, bytes) in cases.iter().enumerate() {
        assert!(preview(bytes, &[]).is_err(), "no-op corruption {case}");
        assert!(
            preview(bytes, &[request(&model, 3)]).is_err(),
            "delete corruption {case}"
        );
    }
    assert!(preview(original, &[request(&model, 3)]).is_ok());
}

#[test]
fn tree_delete_bounds_cancellation_and_expected_rows_leave_original_reusable() {
    let model = database(6, true, true, true, true);
    let row = model.leaves[1].1[1];
    let requests = [request(&model, row)];
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 20_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 20_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            capture(&model.bytes).preview_ace16_deletes(
                "Items",
                &requests,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0usize);
    let expected = capture(&model.bytes)
        .preview_ace16_deletes("Items", &requests, &mut budget(), &|| {
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
        let result =
            capture(&model.bytes).preview_ace16_deletes("Items", &requests, &mut budget(), &|| {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            });
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Cancelled))),
            "checkpoint {checkpoint}/{observed}"
        );
    }
    assert_eq!(
        preview(&model.bytes, &[request(&model, row), request(&model, row)]).err(),
        Some(EditError::Conflict)
    );
    assert_eq!(
        preview(&model.bytes, &[RowDelete::new(8, vec![2, 0])]).err(),
        Some(EditError::Conflict)
    );
    let retry = preview(&model.bytes, &requests).unwrap();
    assert_eq!(retry.candidate_bytes(), expected.candidate_bytes());
    verify(&model, &retry, &[row]);
}
