use super::{base_model, model::*};
use base_model::{entries, number, preview, rows, word};
use tablecraft_ace::Int32Update;

#[test]
fn nonroot_leaf_keeps_old_right_inserts_left_and_repairs_full_parent_bounds() {
    for code in [5, 6] {
        for descending in [false, true] {
            let before = source(code, descending);
            assert_checked(&before);
            let row = if descending { 0 } else { ROWS - 1 };
            let old = row as i32 + 1;
            let new = if descending { i32::MIN } else { i32::MAX };
            let output = preview(&before, &[Int32Update::new(row, old, new)]).unwrap();
            let after = output.candidate_bytes();
            assert_checked(after);
            assert_eq!(after.len(), before.len() + PAGE);
            assert_eq!(output.changed_rows(), 1);
            assert_eq!(number(after, ROOT * PAGE + 20), LAST as u32);
            assert_eq!(after[ROOT * PAGE + 26], 1);
            assert_eq!(number(after, FIRST * PAGE + 16), ADDED as u32);
            assert_eq!(number(after, ADDED * PAGE + 12), FIRST as u32);
            assert_eq!(number(after, ADDED * PAGE + 16), LAST as u32);
            assert_eq!(number(after, LAST * PAGE + 12), ADDED as u32);
            assert_eq!(number(after, LAST * PAGE + 16), 0);
            let expected = expected(descending);
            let middle = FIRST_COUNT + (ROWS - FIRST_COUNT) / 2;
            assert_eq!(entries(after, FIRST), expected[..FIRST_COUNT]);
            assert_eq!(entries(after, ADDED), expected[FIRST_COUNT..middle]);
            assert_eq!(entries(after, LAST), expected[middle..]);
            let mut first = expected[FIRST_COUNT - 1].clone();
            first.extend((FIRST as u32).to_be_bytes());
            let mut second = expected[middle - 1].clone();
            second.extend((ADDED as u32).to_be_bytes());
            assert_eq!(entries(after, ROOT), vec![first, second]);
            let global = bitmap_offset(&before, 1, ADDED);
            let owned = bitmap_offset(&before, 14, ADDED);
            assert_eq!(after[global], before[global] & !4);
            assert_eq!(after[owned], before[owned] | 4);
            assert_eq!(word(after, ADDED * PAGE + 24), 0);
            assert_eq!(word(after, LAST * PAGE + 24), 1);
            let old_rows = rows(&before);
            let actual = rows(after);
            for (position, record) in old_rows.iter().enumerate() {
                if position != row {
                    assert_eq!(&actual[position], record);
                }
            }
        }
    }
}

#[test]
fn nonroot_empty_and_noop_requests_keep_every_original_byte() {
    let before = source(6, false);
    for requests in [vec![], vec![Int32Update::new(499, 500, 500)]] {
        assert_eq!(
            preview(&before, &requests).unwrap().candidate_bytes(),
            before
        );
    }
    let fitted = preview(&before, &[Int32Update::new(499, 500, 501)]).unwrap();
    assert_eq!(fitted.candidate_bytes().len(), before.len());
    assert_checked(fitted.candidate_bytes());
    assert_eq!(
        base_model::index_rows(fitted.candidate_bytes(), "ScoreKey"),
        base_model::index_rows(fitted.candidate_bytes(), "ScoreAlias")
    );
}
