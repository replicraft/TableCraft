use super::{model::*, support};
use std::cell::Cell;
use tablecraft_ace::{EditError, InspectError, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn nonrestrict_unknown_and_unenforced_declarations_never_gain_empty_request_authority() {
    for (flags, action) in [(0x1000, 1), (0x2000, 2), (2, 0), (4, 0), (0x80000000, 0)] {
        let mut model = database(6, false, false, false);
        relationships(&mut model, flags);
        for index in [0, 1] {
            let logical = model.logical_start + usize::from(index == 1) * LOGICAL_BYTES;
            model.bytes[definition(index) * PAGE_BYTES + logical + DELETE_ACTION] = action;
        }
        for deleted in [vec![], vec![2]] {
            assert!(candidate(&model, 0, &deleted).is_err());
        }
    }

    // An intentionally unenforced relationship has ordinary indexes, not
    // contradictory native enforcement links. Read/check acceptance alone
    // must not grant DELETE authority.
    let mut unenforced = database(6, false, false, false);
    relationships(&mut unenforced, 2);
    for index in [0, 1] {
        let logical = unenforced.logical_start + usize::from(index == 1) * LOGICAL_BYTES;
        let start = definition(index) * PAGE_BYTES + logical;
        unenforced.bytes[start + FOREIGN_ROLE..start + INDEX_KIND + 1].fill(0);
    }
    for target in [0, 1] {
        let checked = tablecraft_ace::check_table(
            &mut std::io::Cursor::new(&unenforced.bytes),
            tablecraft_model::CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == tablecraft_ace::IndexCheckStatus::Checked)
        );
        let report = tablecraft_ace::check_table_constraints(
            &mut std::io::Cursor::new(&unenforced.bytes),
            tablecraft_model::CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.relationships().len(), 1);
        assert_eq!(
            report.relationships()[0].status(),
            tablecraft_ace::RelationshipCheckStatus::NotEnforced
        );
        for deleted in [vec![], vec![2]] {
            assert_eq!(
                candidate(&unenforced, target, &deleted).err(),
                Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
            );
        }
    }
}

#[test]
fn reciprocal_catalog_key_and_later_source_corruption_are_not_hidden_by_child_deletion() {
    let original = database(6, false, true, false);
    let mut cases = Vec::new();
    for (index, field, value) in [
        (0, FOREIGN_ROLE, 1),
        (0, FOREIGN_INDEX, 7),
        (0, FOREIGN_TABLE, 0),
        (1, DELETE_ACTION, 1),
        (0, INDEX_KIND, 0),
    ] {
        let mut bytes = original.bytes.clone();
        let logical = original.logical_start + usize::from(index == 1) * LOGICAL_BYTES;
        bytes[definition(index) * PAGE_BYTES + logical + field] = value;
        cases.push(bytes);
    }
    let mut wrong_owner = original.bytes.clone();
    const OWNER: usize = 4;
    support::put32(&mut wrong_owner, root(0) * PAGE_BYTES + OWNER, 7);
    cases.push(wrong_owner);
    for bytes in cases {
        let before = bytes.clone();
        let image = capture(&bytes);
        let result = image.preview_ace16_deletes(
            "Items",
            &requests(&original, 0, &[2]),
            &mut budget(),
            &|| false,
        );
        assert!(result.is_err());
        assert_eq!(bytes, before);
    }
    let mut bad_keys = database(6, true, false, false);
    const KEY_COLUMN: usize = 4;
    const PHYSICAL_BYTES: usize = 52;
    let physical = bad_keys.logical_start - PHYSICAL_BYTES;
    support::put16(
        &mut bad_keys.bytes,
        definition(0) * PAGE_BYTES + physical + KEY_COLUMN,
        2,
    );
    assert!(candidate(&bad_keys, 0, &[]).is_err());
}

#[test]
fn stale_duplicate_quota_and_cancel_failures_leave_retry_and_noop_update_contracts_intact() {
    let model = database(6, false, true, true);
    let expected = requests(&model, 0, &[0]);
    let mut stale = requests(&model, 0, &[0]);
    stale[0] = tablecraft_ace::RowDelete::new(0, model.records[0][1].clone());
    assert!(
        capture(&model.bytes)
            .preview_ace16_deletes("Items", &stale, &mut budget(), &|| false)
            .is_err()
    );
    assert!(candidate(&model, 0, &[0, 0]).is_err());
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        let result = capture(&model.bytes).preview_ace16_deletes(
            "Items",
            &expected,
            &mut allowance,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let observed = Cell::new(0);
    let complete = capture(&model.bytes)
        .preview_ace16_deletes("Items", &expected, &mut budget(), &|| {
            observed.set(observed.get() + 1);
            false
        })
        .unwrap();
    for threshold in [0, observed.get() / 2, observed.get() - 1] {
        let calls = Cell::new(0);
        let result =
            capture(&model.bytes).preview_ace16_deletes("Items", &expected, &mut budget(), &|| {
                let previous = calls.get();
                calls.set(previous + 1);
                previous == threshold
            });
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
    verify(&model, &complete, 0, &[0]);
    let noop = capture(&model.bytes)
        .preview_ace16_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(noop.candidate_bytes(), model.bytes);
    let normal =
        capture(&model.bytes).plan_int32_updates("Items", "Score", &[], &mut budget(), &|| false);
    assert!(matches!(
        normal,
        Err(EditError::Unsupported(WriteBlocker::Format))
    ));
}

#[test]
fn self_relationships_remain_outside_cross_table_delete_admission() {
    let bytes = super::self_fixture::database(6, false, false);
    let mut record = 2_u16.to_le_bytes().to_vec();
    record.extend(3_i32.to_le_bytes());
    record.extend(1_i32.to_le_bytes());
    record.push(3);
    for requests in [vec![], vec![tablecraft_ace::RowDelete::new(2, record)]] {
        let result =
            capture(&bytes).preview_ace16_deletes("Items", &requests, &mut budget(), &|| false);
        assert!(matches!(
            result,
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
    }
}
