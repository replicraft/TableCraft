use super::{fixture, foreign_fixture, index_fixture, model::*, support};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, InspectError, Int32Update, NativeImage, RowDelete, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[test]
fn expected_record_identity_duplicate_ordinals_and_missing_rows_are_conflicts() {
    let bytes = ace(fixture::database(), 6);
    let original = request(1, false);
    assert_eq!(original.row(), 1);
    assert_eq!(
        original.expected_bytes(),
        [2, 0, 2, 0, 0, 0, 20, 0, 0, 0, 3]
    );

    let mut stale = original.expected_bytes().to_vec();
    stale[6] = 99;
    for requests in [
        vec![RowDelete::new(1, stale)],
        vec![request(1, false), request(1, false)],
        vec![request(3, false)],
        vec![RowDelete::new(1, vec![])],
    ] {
        assert_eq!(preview(&bytes, &requests).err(), Some(EditError::Conflict));
    }
    assert!(preview(&bytes, &[RowDelete::new(1, vec![0; 4097])]).is_err());
    assert_eq!(table_rows(&bytes).len(), 3);
    assert!(preview(&bytes, &[request(1, false)]).is_ok());
}

#[test]
fn empty_requests_do_not_bypass_format_properties_or_ordinary_write_gates() {
    for bytes in [fixture::database(), ace(fixture::database(), 2)] {
        assert_eq!(
            preview(&bytes, &[]).err(),
            Some(EditError::Unsupported(WriteBlocker::Format))
        );
    }
    let bytes = ace(fixture::with_rule("UnknownBehavior", "required"), 6);
    assert_eq!(
        preview(&bytes, &[]).err(),
        Some(EditError::Unsupported(WriteBlocker::Properties))
    );
    let bytes = ace(fixture::database(), 6);
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 21)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Format))
    );
    assert_eq!(preview(&bytes, &[]).unwrap().candidate_bytes(), bytes);
}

#[test]
fn ignored_null_relationship_actions_and_leaf_exhaustion_remain_outside_delete_scope() {
    let mut ignored = ace(index_fixture::database(false, false, false), 6);
    ignored[10 * 4096 + 189] |= 2;
    for bytes in [
        ignored,
        foreign_fixture::database(6, false, false),
        foreign_fixture::database(6, false, true),
    ] {
        for requests in [vec![], vec![request(1, false)]] {
            assert!(matches!(
                preview(&bytes, &requests),
                Err(EditError::Unsupported(_))
            ));
        }
    }

    let node = ace(index_fixture::node_database(), 6);
    assert_eq!(preview(&node, &[]).unwrap().candidate_bytes(), node);
    assert!(preview(&node, &[request(0, false)]).is_ok());
    // The right leaf in the original independent geometry has just one entry.
    // Deleting it still requires the separately unsupported contraction path.
    assert!(matches!(
        preview(&node, &[request(2, false)]),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
}

#[test]
fn corrupt_owner_slot_key_mask_and_live_count_never_produce_a_candidate() {
    let original = ace(index_fixture::database(false, false, false), 6);
    let mut cases = Vec::new();

    let mut owner = original.clone();
    support::put32(&mut owner, 12 * 4096 + 4, 99);
    cases.push(owner);

    let mut slot = original.clone();
    support::put16(&mut slot, 12 * 4096 + 16, 1);
    cases.push(slot);

    let mut key = original.clone();
    // The first full entry is nine bytes: marker, four key bytes, four reference bytes.
    key[13 * 4096 + 480 + 8] = 99;
    cases.push(key);

    let mut mask = original.clone();
    mask[13 * 4096 + 27] ^= 1 << 4;
    cases.push(mask);

    let mut count = original.clone();
    support::put32(&mut count, 10 * 4096 + 16, 4);
    cases.push(count);

    for (case, bytes) in cases.iter().enumerate() {
        assert!(
            preview(bytes, &[request(1, false)]).is_err(),
            "corrupt case {case}"
        );
        assert!(preview(bytes, &[]).is_err(), "no-op corrupt case {case}");
    }
    assert!(preview(&original, &[request(1, false)]).is_ok());
}

#[test]
fn independent_quota_pools_and_cancellation_expose_no_candidate_and_allow_retry() {
    let bytes = ace(index_fixture::database(true, true, true), 6);
    let requests = [request(0, true), request(2, true)];
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 10_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 10_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            capture(&bytes).preview_ace16_deletes("Items", &requests, &mut allowance, &|| false),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }

    let calls = Cell::new(0usize);
    let complete = capture(&bytes)
        .preview_ace16_deletes("Items", &requests, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(complete.deleted_rows(), 2);
    let observed = calls.get();
    assert!(observed > 4);
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
            capture(&bytes).preview_ace16_deletes("Items", &requests, &mut budget(), &|| {
                let current = calls.get();
                calls.set(current + 1);
                current == checkpoint
            });
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Cancelled))),
            "checkpoint {checkpoint}/{observed}"
        );
    }
    assert!(matches!(
        capture(&bytes).preview_ace16_deletes("Items", &[], &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(
        preview(&bytes, &requests).unwrap().candidate_bytes(),
        complete.candidate_bytes()
    );
    assert_eq!(table_rows(&bytes).len(), 3);
}

#[test]
fn new_delete_preview_requires_subversion_zero_while_encoded_capture_stays_separate() {
    for code in [5, 6] {
        // Synthetic header derivatives establish admission gates only; they are
        // not native-authored fixtures or qualification of subversion semantics.
        let mut bytes = ace(fixture::database(), code);
        bytes[21] = 1;
        let image = capture(&bytes);
        assert_eq!(image.catalog().header().subversion(), 1);
        assert!(!image.catalog().header().encoded_pages());
        for requests in [vec![], vec![request(1, false)]] {
            assert_eq!(
                preview(&bytes, &requests).err(),
                Some(EditError::Unsupported(WriteBlocker::Format))
            );
        }

        // The existing fixed Int32 preview shares the documented preview gate;
        // its no-op request must not turn broader read admission into writes.
        assert_eq!(
            capture(&bytes)
                .preview_ace16_int32_updates("Items", "Score", &[], &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Format))
        );

        let mut encoded = ace(fixture::database(), code);
        encoded[62] ^= 1;
        assert_eq!(
            NativeImage::capture(
                &mut Cursor::new(&encoded),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(InspectError::Encoded)
        );
    }
}
