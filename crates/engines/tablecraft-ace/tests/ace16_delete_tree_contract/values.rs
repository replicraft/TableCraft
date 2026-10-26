use super::model::*;

#[test]
fn authored_multi_entry_leaves_delete_in_place_under_both_node_forms_and_directions() {
    for code in [5, 6] {
        for descending in [false, true] {
            for bounded in [false, true] {
                for deep in [false, true] {
                    for alias in [false, true] {
                        let model = database(code, descending, bounded, deep, alias);
                        let no_op = preview(&model.bytes, &[]).unwrap();
                        assert_eq!(no_op.candidate_bytes(), model.bytes);
                        let first = model.ordered[0];
                        let middle = model.ordered[4];
                        let last = *model.ordered.last().unwrap();
                        let maxima = model
                            .leaves
                            .iter()
                            .map(|leaf| *leaf.1.last().unwrap())
                            .collect::<Vec<_>>();
                        for deleted in [vec![first], vec![middle], vec![last], maxima] {
                            let requests = deleted
                                .iter()
                                .map(|&row| request(&model, row))
                                .collect::<Vec<_>>();
                            let candidate = preview(&model.bytes, &requests).unwrap();
                            verify(&model, &candidate, &deleted);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn separator_replacement_preserves_null_and_duplicate_key_row_reference_boundaries() {
    for bounded in [false, true] {
        let model = database(6, false, bounded, true, true);
        // Original first leaf ends at NULL/ref1, its predecessor is NULL/ref0.
        // The second leaf ends at 10/ref4, its predecessor is 10/ref3.
        assert_eq!(model.leaves[0].1, [0, 1]);
        assert_eq!(model.leaves[1].1, [2, 3, 4]);
        let deleted = [1, 4, 7];
        let requests = deleted.map(|row| request(&model, row));
        let candidate = preview(&model.bytes, &requests).unwrap();
        verify(&model, &candidate, &deleted);
        let before = &model.bytes;
        let after = candidate.candidate_bytes();
        // Intermediate node17's first stored bound is NULL, then a four-byte row
        // reference. Its key stays zero, while ref12:1 must become ref12:0.
        assert_eq!(before[17 * 4096 + 480], 0);
        assert_eq!(after[17 * 4096 + 480], 0);
        assert_eq!(&before[17 * 4096 + 481..17 * 4096 + 485], &[0, 0, 12, 1]);
        assert_eq!(&after[17 * 4096 + 481..17 * 4096 + 485], &[0, 0, 12, 0]);
    }
}
