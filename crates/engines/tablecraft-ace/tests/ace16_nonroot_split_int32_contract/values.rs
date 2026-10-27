use super::*;

#[test]
fn first_middle_last_splits_preserve_full_images_and_both_parent_forms() {
    for code in [5, 6] {
        for descending in [false, true] {
            for bounded in [false, true] {
                let scores = groups(false);
                let model = source(
                    code,
                    scores.clone(),
                    vec![600; 3],
                    vec![descending],
                    bounded,
                );
                check(&model);
                let cases = if descending {
                    [(1799, i32::MAX, 0), (600, 0x01ff_ffff, 1), (0, i32::MIN, 2)]
                } else {
                    [(0, i32::MIN, 0), (600, 0x01ff_ffff, 1), (1799, i32::MAX, 2)]
                };
                for (row, value, original_leaf) in cases {
                    let result =
                        run(&model, Int32Update::new(row, scores[row].unwrap(), value)).unwrap();
                    let (expected, facts) = expected(&model, row, value);
                    assert_eq!(facts.len(), 1);
                    assert_eq!(facts[0].old_right, model.leaves[0][original_leaf]);
                    assert_eq!(facts[0].new_left, model.eof);
                    assert_eq!(facts[0].left.len(), 300);
                    assert_eq!(facts[0].right.len(), 300);
                    assert_eq!(result.candidate_bytes(), expected);
                    verify_values(&model, result.candidate_bytes(), row, value);
                }
            }
        }
    }
}

#[test]
fn tied_separator_uses_the_complete_row_identity_across_both_new_halves() {
    for descending in [false, true] {
        let model = source(6, groups(true), vec![600; 3], vec![descending], false);
        check(&model);
        let row = 600;
        let old = model.scores[row].unwrap();
        let value = 0x01ff_ffff;
        let result = run(&model, Int32Update::new(row, old, value)).unwrap();
        let (expected, facts) = expected(&model, row, value);
        assert_eq!(result.candidate_bytes(), expected);
        let split = &facts[0];
        let left_max = split.left.last().unwrap();
        let right_min = split.right.first().unwrap();
        assert_eq!(&left_max[..5], &right_min[..5]);
        assert_ne!(&left_max[5..], &right_min[5..]);
        let bounds = decoded(result.candidate_bytes(), ROOTS[0]);
        let bound = bounds
            .iter()
            .find(|entry| entry.ends_with(&(split.new_left as u32).to_be_bytes()))
            .unwrap();
        assert_eq!(&bound[..9], left_max);
    }
}

#[test]
fn real_aliases_and_two_physical_owners_compose_one_future_global_byte() {
    let model = source(6, groups(false), vec![600; 3], vec![false, true], false);
    check(&model);
    let value = 0x01ff_ffff;
    let result = run(
        &model,
        Int32Update::new(600, model.scores[600].unwrap(), value),
    )
    .unwrap();
    let (expected, facts) = expected(&model, 600, value);
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].physical, 0);
    assert_eq!(facts[1].physical, 1);
    assert_eq!(facts[0].new_left, model.eof);
    assert_eq!(facts[1].new_left, model.eof + 1);
    assert_eq!(model.eof % 8, 0);
    let global = root_model::map_byte(&model.bytes, 1, model.eof);
    assert_eq!(model.bytes[global], 0xff);
    assert_eq!(result.candidate_bytes()[global], 0xfc);
    assert_eq!(result.candidate_bytes(), expected);
    for (physical, &map_page) in MAPS.iter().enumerate() {
        let map = root_model::map_byte(&model.bytes, map_page, model.eof);
        assert_eq!(result.candidate_bytes()[map], 1 << physical);
    }
    verify_values(&model, result.candidate_bytes(), 600, value);
}

#[test]
fn nullable_nonoverflowing_peers_never_allocate_or_relink() {
    let mut scores = (1..=900).map(Some).collect::<Vec<_>>();
    scores[0] = None;
    for descending in [false, true] {
        let model = source(5, scores.clone(), vec![300; 3], vec![descending], true);
        check(&model);
        let result = run(&model, Int32Update::new(1, 2, 3)).unwrap();
        let (expected, facts) = expected(&model, 1, 3);
        assert!(facts.is_empty());
        assert_eq!(result.candidate_bytes().len(), model.bytes.len());
        assert_eq!(result.candidate_bytes(), expected);
        verify_values(&model, result.candidate_bytes(), 1, 3);
    }
}
