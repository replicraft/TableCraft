use super::model::*;
use tablecraft_ace::{EditError, WriteBlocker};

#[test]
fn omitted_roots_have_independent_effective_subsets_in_every_physical_index() {
    for code in [5, 6] {
        let model = database(code);
        checked(&model.bytes, 6);
        let properties = tablecraft_ace::inspect_table_properties(
            &mut std::io::Cursor::new(&model.bytes),
            tablecraft_model::CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            properties
                .group(tablecraft_ace::PropertyGroupKind::Column, "Score")
                .is_some()
        );
        assert_eq!(model.entries.each_ref().map(Vec::len), [6, 4, 3, 5]);
        for deleted in [
            vec![],
            vec![0],
            vec![1],
            vec![2],
            vec![0, 1, 3],
            vec![1, 2, 4],
        ] {
            let output = candidate(&model, &deleted).unwrap();
            verify(&model, &output, &deleted);
        }
    }
}

#[test]
fn empty_ignored_tree_retires_descendants_even_while_null_table_roots_remain() {
    for code in [5, 6] {
        let model = database(code);
        let deleted = [2, 3, 4, 5];
        let output = candidate(&model, &deleted).unwrap();
        verify(&model, &output, &deleted);
        let bytes = output.candidate_bytes();
        let root = ROOTS[1] * PAGE_BYTES;
        assert_eq!(&bytes[root..root + 2], &[4, 1]);
        assert_eq!(bytes[root + HEIGHT], 0);
        assert_eq!(&bytes[root + PREVIOUS..root + PREFIX], &[0; 12]);
        assert!(decoded_entries(bytes, ROOTS[1]).is_empty());
        // Inline owned-map row 1 now contains exactly the unchanged physical root.
        let slot_offset = OWNED_MAP * PAGE_BYTES + SLOT_DIRECTORY + 2;
        let start =
            u16::from_le_bytes(bytes[slot_offset..slot_offset + 2].try_into().unwrap()) as usize;
        let map = OWNED_MAP * PAGE_BYTES + (start & 0x1fff) + 5;
        let mut expected_owned = [0; 4];
        expected_owned[ROOTS[1] / 8] |= 1 << (ROOTS[1] % 8);
        assert_eq!(&bytes[map..map + expected_owned.len()], &expected_owned);
        for leaf in LEAVES {
            let offset = leaf * PAGE_BYTES;
            assert_eq!(&bytes[offset..offset + 4], &[0; 4]);
            assert_eq!(
                &bytes[offset + 4..offset + PAGE_BYTES],
                &model.bytes[offset + 4..offset + PAGE_BYTES]
            );
            let free_map = 2 * PAGE_BYTES - 64;
            assert_ne!(bytes[free_map + leaf / 8] & (1 << (leaf % 8)), 0);
        }
        // Removing another omitted A root must leave the already empty tree exact.
        let follow_up = capture(bytes)
            .preview_ace16_deletes(
                "Items",
                &[tablecraft_ace::RowDelete::new(0, model.records[0].clone())],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        checked(follow_up.candidate_bytes(), 1);
        assert_eq!(
            &follow_up.candidate_bytes()[root..root + PAGE_BYTES],
            &bytes[root..root + PAGE_BYTES]
        );
    }
}

#[test]
fn delete_all_counts_every_row_and_preserves_preview_update_refusals() {
    let model = database(6);
    let output = candidate(&model, &[0, 1, 2, 3, 4, 5]).unwrap();
    verify(&model, &output, &[0, 1, 2, 3, 4, 5]);
    for root in ROOTS {
        assert!(decoded_entries(output.candidate_bytes(), root).is_empty());
    }
    let result = capture(&model.bytes).preview_ace16_int32_updates(
        "Items",
        "Score",
        &[],
        &mut budget(),
        &|| false,
    );
    assert!(matches!(
        result,
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));
    for index in ["IgnoredA", "AliasA", "UniqueB", "Mixed"] {
        let result = tablecraft_ace::read_table_index(
            &mut std::io::Cursor::new(&model.bytes),
            tablecraft_model::CatalogScope::new(1),
            "Items",
            index,
            &mut budget(),
            &|| false,
        );
        assert!(matches!(
            result,
            Err(tablecraft_ace::InspectError::Unsupported)
        ));
    }
}
