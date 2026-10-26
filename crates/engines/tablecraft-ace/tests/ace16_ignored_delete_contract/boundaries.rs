use super::{model::*, support};
use std::cell::Cell;
use tablecraft_ace::{EditError, InspectError};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn complete_before_membership_rejects_missing_extra_duplicate_and_wrong_keys() {
    let model = database(6);
    let mut cases = Vec::new();
    let mut missing = model.entries[3].clone();
    missing.retain(|entry| slot(entry) != 1);
    cases.push(missing);
    let mut extra = model.entries[3].clone();
    extra.push(vec![0, 255, 0, 0, 12, 0]);
    extra.sort();
    cases.push(extra);
    let mut duplicate = model.entries[3].clone();
    duplicate.push(duplicate[0].clone());
    duplicate.sort();
    cases.push(duplicate);
    let mut wrong = model.entries[3].clone();
    wrong[0][1] ^= 1;
    wrong.sort();
    cases.push(wrong);
    for entries in cases {
        let mut invalid = database(6);
        let root = ROOTS[3] * PAGE_BYTES;
        invalid.bytes[root..root + PAGE_BYTES].copy_from_slice(&page(0, &entries, 0, 0, 0));
        let before = invalid.bytes.clone();
        assert!(candidate(&invalid, &[0]).is_err());
        assert_eq!(invalid.bytes, before);
    }
    // The requested NULL/NULL row is omitted from this physical index. Its
    // corrupt ownership still fails before an effective-empty shortcut.
    let mut invalid = database(6);
    support::put32(&mut invalid.bytes, ROOTS[2] * PAGE_BYTES + OWNER, 7);
    assert!(candidate(&invalid, &[0]).is_err());
}

#[test]
fn stale_duplicate_requests_and_later_index_failure_never_publish_a_prefix() {
    let model = database(6);
    let mut stale = requests(&model, &[0]);
    stale[0] = tablecraft_ace::RowDelete::new(0, model.records[1].clone());
    assert!(
        capture(&model.bytes)
            .preview_ace16_deletes("Items", &stale, &mut budget(), &|| false)
            .is_err()
    );
    assert!(candidate(&model, &[0, 0]).is_err());
    let mut invalid = database(6);
    // Reserved physical flags on the final index cannot be bypassed merely
    // because the selected row is omitted there.
    const PHYSICAL_BYTES: usize = 52;
    const FLAGS: usize = 46;
    invalid.bytes[DEFINITION * PAGE_BYTES + invalid.physical_start + 3 * PHYSICAL_BYTES + FLAGS] |=
        4;
    assert!(candidate(&invalid, &[0]).is_err());
    verify(&model, &candidate(&model, &[0]).unwrap(), &[0]);
}

#[test]
fn cumulative_quotas_and_cancellation_are_atomic_and_retryable() {
    let model = database(6);
    let deleted = [2, 3, 4, 5];
    let expected = requests(&model, &deleted);
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 30_000_000),
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
    for checkpoint in [
        0,
        observed.get() / 3,
        observed.get() * 2 / 3,
        observed.get() - 1,
    ] {
        let calls = Cell::new(0);
        let result =
            capture(&model.bytes).preview_ace16_deletes("Items", &expected, &mut budget(), &|| {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            });
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
    verify(&model, &complete, &deleted);
    assert_eq!(
        candidate(&model, &deleted).unwrap().candidate_bytes(),
        complete.candidate_bytes()
    );
}

#[test]
fn empty_effective_memo_requests_still_require_a_supported_delete_codec() {
    const INDEX_FLAGS: usize = 189;
    const ENCODED_ROOT: usize = 13;
    let mut model = super::encoded_model::null_memo();
    model.bytes[DEFINITION * PAGE_BYTES + INDEX_FLAGS] = 0x82;
    let start = ENCODED_ROOT * PAGE_BYTES;
    model.bytes[start..start + PAGE_BYTES].copy_from_slice(&page(0, &[], 0, 0, 0));
    let report = tablecraft_ace::check_table(
        &mut std::io::Cursor::new(&model.bytes),
        tablecraft_model::CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), 1);
    assert_eq!(
        report.indexes()[0].status(),
        tablecraft_ace::IndexCheckStatus::Checked
    );
    for deleted in [vec![], vec![0]] {
        let result = super::encoded_model::candidate(&model, &deleted);
        assert_eq!(
            result.err(),
            Some(EditError::Unsupported(
                tablecraft_ace::WriteBlocker::IndexedColumn
            ))
        );
    }
}
