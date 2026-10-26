use super::{model::*, tree_model};

#[test]
fn empty_first_middle_and_last_leaves_repair_the_whole_leaf_chain_without_promotion() {
    for code in [5, 6] {
        for descending in [false, true] {
            for bounded in [false, true] {
                for deep in [false, true] {
                    let model = database(code, descending, bounded, deep, true, false, false);
                    for leaf in 0..3 {
                        let deleted = model.tree.leaves[leaf].1.clone();
                        let mut retired = vec![model.tree.leaves[leaf].0];
                        if deep && leaf == 2 {
                            retired.push(18);
                        }
                        let output = candidate(&model, &deleted).unwrap();
                        verify(&model, &output, &deleted, &retired);
                    }
                }
            }
        }
    }
}

#[test]
fn exhausting_an_entire_branch_keeps_nonempty_root_and_single_child_heights() {
    for bounded in [false, true] {
        for descending in [false, true] {
            let model = database(6, descending, bounded, true, true, false, false);
            let deleted = tree_model::subtree_rows(17, &model.tree.leaves, &model.tree.nodes);
            let output = candidate(&model, &deleted).unwrap();
            verify(&model, &output, &deleted, &[15, 16, 17]);
            let after = output.candidate_bytes();
            assert_eq!(after[13 * 4096], 3);
            assert_eq!(after[13 * 4096 + 26], 2);
            assert_eq!(after[18 * 4096], 3);
            assert_eq!(after[18 * 4096 + 26], 1);
        }
    }
}

#[test]
fn delete_all_keeps_root_identity_and_aggregates_same_byte_retirements_once() {
    for bounded in [false, true] {
        for deep in [false, true] {
            let model = database(6, false, bounded, deep, true, false, false);
            let retired = if deep {
                vec![15, 16, 17, 18, 19]
            } else {
                vec![15, 16, 19]
            };
            let output = candidate(&model, &(0..8).collect::<Vec<_>>()).unwrap();
            verify(&model, &output, &(0..8).collect::<Vec<_>>(), &retired);
            let after = output.candidate_bytes();
            assert_eq!(after[13 * 4096], 4);
            assert_eq!(after[13 * 4096 + 26], 0);
            assert_eq!(after[model.owned_bitmap + 1], 0x20);
            assert_eq!(after[model.owned_bitmap + 2], 0);
            assert_eq!(after[model.global_bitmap + 1], 0x80);
            assert_eq!(
                after[model.global_bitmap + 2],
                if deep { 0xff } else { 0xf9 }
            );
            assert_eq!(preview(after, &[]).unwrap().candidate_bytes(), after);
        }
    }
}

#[test]
fn existing_indirect_map_cells_preserve_unrelated_and_beyond_eof_free_bits() {
    for global in [false, true] {
        for owned in [false, true] {
            let model = database(6, false, true, true, true, global, owned);
            let deleted = tree_model::subtree_rows(17, &model.tree.leaves, &model.tree.nodes);
            let output = candidate(&model, &deleted).unwrap();
            verify(&model, &output, &deleted, &[15, 16, 17]);
            assert_eq!(
                preview(&model.tree.bytes, &[]).unwrap().candidate_bytes(),
                model.tree.bytes
            );
        }
    }
}
