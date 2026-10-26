use super::model::*;

#[test]
fn first_middle_and_last_branches_repair_complete_internal_and_leaf_chains() {
    for code in [5, 6] {
        for descending in [false, true] {
            for linked in [false, true] {
                let model = database(code, descending, linked, false);
                for internal in INTERNALS {
                    let deleted = branch(&model, internal);
                    let mut retired = vec![internal];
                    for (page, rows) in &model.base.tree.leaves {
                        if rows.iter().all(|row| deleted.contains(row)) {
                            retired.push(*page);
                        }
                    }
                    let result = candidate(&model, &deleted).unwrap();
                    verify(&model, &result, &deleted, &retired);
                }
            }
        }
    }
}

#[test]
fn singleton_heights_and_tied_full_row_maxima_survive_mixed_child_forms() {
    for descending in [false, true] {
        let model = database(6, descending, true, true);
        for keep in [LEAVES[0], LEAVES[2], LEAVES[3]] {
            let kept = branch(&model, keep);
            let deleted = (0..8).filter(|row| !kept.contains(row)).collect::<Vec<_>>();
            let mut retired = LEAVES
                .iter()
                .copied()
                .filter(|page| *page != keep)
                .collect::<Vec<_>>();
            for internal in INTERNALS {
                if branch(&model, internal)
                    .iter()
                    .all(|row| deleted.contains(row))
                {
                    retired.push(internal);
                }
            }
            let result = candidate(&model, &deleted).unwrap();
            verify(&model, &result, &deleted, &retired);
            assert_eq!(
                result.candidate_bytes()[ROOT as usize * PAGE_BYTES + HEIGHT],
                2
            );
        }
    }
    let model = database(6, false, true, false);
    // Rows 4 and 5 have the same authored key10. Removing the latter must
    // replace the row-reference component of the parent's maximum too.
    assert_eq!(model.base.tree.keys[4], model.base.tree.keys[5]);
    let result = candidate(&model, &[5]).unwrap();
    verify(&model, &result, &[5], &[]);
}

#[test]
fn delete_all_converts_the_same_root_and_preserves_every_retired_body() {
    for code in [5, 6] {
        for indirect in [false, true] {
            let model = database(code, false, true, indirect);
            let deleted = (0..8).collect::<Vec<_>>();
            let retired = LEAVES.into_iter().chain(INTERNALS).collect::<Vec<_>>();
            let result = candidate(&model, &deleted).unwrap();
            verify(&model, &result, &deleted, &retired);
            let root = ROOT as usize * PAGE_BYTES;
            assert_eq!(result.candidate_bytes()[root], 4);
            assert_eq!(result.candidate_bytes()[root + HEIGHT], 0);
            assert_eq!(
                &result.candidate_bytes()[root + PREVIOUS..root + TAIL + 4],
                &[0; 12]
            );
            let no_op = preview(result.candidate_bytes(), &[]).unwrap();
            assert_eq!(no_op.candidate_bytes(), result.candidate_bytes());
        }
    }
}
