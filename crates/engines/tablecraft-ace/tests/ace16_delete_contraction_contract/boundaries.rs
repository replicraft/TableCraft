use super::{model::*, support};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, InspectError, check_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[test]
fn already_free_targets_and_missing_or_unrepresented_global_maps_refuse_atomically() {
    let model = database(6, false, true, true, false, false, false);
    let deleted = model.tree.leaves[0].1.clone();
    let requests = requests(&model, &deleted);
    let original = &model.tree.bytes;
    let mut cases = Vec::new();

    let mut free = original.clone();
    free[model.global_bitmap + 15 / 8] |= 1 << (15 % 8);
    cases.push(free);

    let mut root_free = original.clone();
    root_free[model.global_bitmap + 13 / 8] |= 1 << (13 % 8);
    cases.push(root_free);

    let mut unknown = original.clone();
    unknown[8192 - 69] = 99;
    cases.push(unknown);

    let mut missing = original.clone();
    missing[4096..8192].copy_from_slice(&support::data_page(1, &[(0, vec![1, 0, 0, 0, 0])]));
    cases.push(missing);

    let mut uncovered = original.clone();
    uncovered[4096..8192].copy_from_slice(&support::data_page(1, &[(0, vec![0, 16, 0, 0, 0, 0])]));
    cases.push(uncovered);

    let mut indirect = original.clone();
    indirect[4096..8192].copy_from_slice(&support::data_page(1, &[(0, vec![1, 99, 0, 0, 0])]));
    cases.push(indirect);

    let mut deleted_map = original.clone();
    let word = u16::from_le_bytes(deleted_map[4096 + 14..4096 + 16].try_into().unwrap());
    support::put16(&mut deleted_map, 4096 + 14, word | 0x8000);
    cases.push(deleted_map);

    for (case, bytes) in cases.iter().enumerate() {
        assert!(preview(bytes, &requests).is_err(), "global map case {case}");
    }
    assert!(candidate(&model, &deleted).is_ok());
}

#[test]
fn map_metadata_or_data_aliases_and_unsupported_internal_links_cannot_retire_storage() {
    let model = database(6, false, true, true, true, false, false);
    let branch = model.tree.leaves[..2]
        .iter()
        .flat_map(|leaf| leaf.1.iter().copied())
        .collect::<Vec<_>>();
    let requests = requests(&model, &branch);
    let mut cases = Vec::new();

    let mut data_overlap = model.tree.bytes.clone();
    // Add the first index leaf to the table-owned data map. Complete source
    // validation must reject this before it can become retirement authority.
    data_overlap[12 * 4096 - 1] |= 0x80;
    cases.push(data_overlap);

    let mut missing_owner = model.tree.bytes.clone();
    missing_owner[model.owned_bitmap + 1] &= !0x80;
    cases.push(missing_owner);

    let mut map_alias = model.tree.bytes.clone();
    support::put32(&mut map_alias, 10 * 4096 + 177, 1 << 8);
    cases.push(map_alias);

    let mut metadata_child = model.tree.bytes.clone();
    // Original root's first child reference follows a nine-byte key/reference.
    metadata_child[13 * 4096 + 489..13 * 4096 + 493].copy_from_slice(&10_u32.to_be_bytes());
    cases.push(metadata_child);

    let mut linked_internal = model.tree.bytes.clone();
    support::put32(&mut linked_internal, 17 * 4096 + 12, 13);
    cases.push(linked_internal);

    for (case, bytes) in cases.iter().enumerate() {
        assert!(
            preview(bytes, &requests).is_err(),
            "protected storage case {case}"
        );
    }
    assert!(candidate(&model, &branch).is_ok());
}

#[test]
fn contraction_quotas_cancellation_and_reuse_never_publish_a_partial_retirement() {
    let model = database(6, false, true, true, true, true, true);
    let deleted = (0..8).collect::<Vec<_>>();
    let requests = requests(&model, &deleted);
    for mut allowance in [
        ResourceBudget::new(0, 100_000, 20_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 20_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            capture(&model.tree.bytes).preview_ace16_deletes(
                "Items",
                &requests,
                &mut allowance,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }
    let calls = Cell::new(0usize);
    let expected = capture(&model.tree.bytes)
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
        let result = capture(&model.tree.bytes).preview_ace16_deletes(
            "Items",
            &requests,
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
    let retry = candidate(&model, &deleted).unwrap();
    assert_eq!(retry.candidate_bytes(), expected.candidate_bytes());
    verify(&model, &retry, &deleted, &[15, 16, 17, 18, 19]);
}

#[test]
fn ordinary_user_owned_map_shaped_records_do_not_authorize_retirement() {
    for indirect_owned in [false, true] {
        let model = database(6, false, true, true, true, false, indirect_owned);
        let deleted = model.tree.leaves[0].1.clone();
        let requests = requests(&model, &deleted);
        for (container, owner) in [(14, 10), (14, 11), (1, 0), (1, 10)] {
            let mut bytes = model.tree.bytes.clone();
            support::put32(&mut bytes, container * 4096 + 4, owner);
            // Existing read checks accept the same map contents regardless of
            // container owner. Read support must not become patch authority.
            check_table(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                preview(&bytes, &requests).is_err(),
                "container {container}, owner {owner}"
            );
        }
        assert!(candidate(&model, &deleted).is_ok());
    }

    let model = database(6, false, true, true, true, true, true);
    let deleted = model.tree.leaves[0].1.clone();
    for page in [11, 20, 21] {
        let mut bytes = model.tree.bytes.clone();
        // A map container or indirect bitmap is live storage, even though
        // ordinary table checks do not consult its global-free bit.
        bytes[model.global_bitmap + page / 8] |= 1 << (page % 8);
        check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            preview(&bytes, &requests(&model, &deleted)).is_err(),
            "free storage page {page}"
        );
    }
    assert!(candidate(&model, &deleted).is_ok());
}
