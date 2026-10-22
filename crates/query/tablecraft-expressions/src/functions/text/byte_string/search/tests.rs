use super::*;
use proptest::prelude::*;
use std::cell::Cell;

#[test]
fn binary_search_counts_bytes_and_retains_unaligned_matches() {
    let text = ByteString::from_owned(vec![65, 0, 45, 78, 62, 216, 128, 221]).unwrap();
    for (needle, start, expected) in [
        (vec![0], 1, 2),
        (vec![78, 62], 1, 4),
        (vec![62, 216], 1, 5),
        (vec![45, 78], 4, 0),
        (vec![], 4, 4),
        (vec![], 9, 0),
        (vec![255], 1, 0),
    ] {
        let needle = ByteString::from_owned(needle).unwrap();
        assert_eq!(find_binary(&text, &needle, start, || Ok(())), Ok(expected));
    }
    let empty = ByteString::from_owned(vec![]).unwrap();
    assert_eq!(find_binary(&empty, &empty, 1, || Ok(())), Ok(0));
    assert_eq!(
        find_binary(&text, &empty, 0, || Ok(())),
        Err(EvalError::Domain)
    );
    assert_eq!(
        find_binary(&text, &empty, 1, || Err(EvalError::Cancelled)),
        Err(EvalError::Cancelled)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn byte_search_matches_independent_slice_positions(
        haystack in prop::collection::vec(0u8..8, 0..100),
        needle in prop::collection::vec(0u8..8, 0..10),
        start in 1i32..120,
    ) {
        let expected = if haystack.is_empty() || start as usize > haystack.len() {
            0
        } else if needle.is_empty() {
            start
        } else {
            ((start - 1) as usize..haystack.len()).find(|offset| {
                haystack.get(*offset..offset.saturating_add(needle.len())) == Some(needle.as_slice())
            }).map_or(0, |offset| offset as i32 + 1)
        };
        let haystack = ByteString::from_owned(haystack).unwrap();
        let needle = ByteString::from_owned(needle).unwrap();
        prop_assert_eq!(find_binary(&haystack, &needle, start, || Ok(())), Ok(expected));
    }
}

#[test]
fn binary_search_checks_every_compared_byte_before_continuing() {
    let haystack = ByteString::from_owned(vec![1; 30]).unwrap();
    let needle = ByteString::from_owned(vec![1, 1, 1, 1, 2]).unwrap();
    let checkpoints = Cell::new(0);
    find_binary(&haystack, &needle, 1, || {
        checkpoints.set(checkpoints.get() + 1);
        Ok(())
    })
    .unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        assert_eq!(
            find_binary(&haystack, &needle, 1, || {
                checkpoints.set(checkpoints.get() + 1);
                if checkpoints.get() == stop {
                    Err(EvalError::Cancelled)
                } else {
                    Ok(())
                }
            }),
            Err(EvalError::Cancelled)
        );
    }
}
