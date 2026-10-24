use super::*;
use proptest::prelude::*;
use tablecraft_expressions::functions::text::byte_string::{ByteString, find_binary};

fn oracle(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.is_empty() {
        return None;
    }
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn production(haystack: &[u8], needle: &[u8]) -> (Option<usize>, u64) {
    let haystack = ByteString::from_owned(haystack.to_vec()).unwrap();
    let needle = ByteString::from_owned(needle.to_vec()).unwrap();
    let mut callbacks = 0;
    let found = find_binary(&haystack, &needle, 1, || {
        callbacks += 1;
        Ok(())
    })
    .unwrap();
    ((found != 0).then(|| found as usize - 1), callbacks)
}

#[test]
fn byte_edges_overlap_and_long_fallback_match_independent_positions() {
    for size in [1, 2, 3, 16, 31, 32, 33, 512] {
        let mut needle = vec![0xff; size];
        needle[0] = 0xd8;
        let length = BLOCK_BYTES * 2 + size;
        for offset in [
            0,
            1,
            BLOCK_BYTES - size,
            BLOCK_BYTES - 1,
            BLOCK_BYTES,
            BLOCK_BYTES + 1,
        ] {
            let mut haystack = vec![0; length];
            haystack[offset..offset + size].copy_from_slice(&needle);
            for search in [scalar as Search, bounded, reference] {
                assert_eq!(
                    search(&haystack, &needle, &mut Meter::default()),
                    Ok(Some(offset))
                );
            }
            assert_eq!(oracle(&haystack, &needle), Some(offset));
        }
    }
    for (haystack, needle) in [
        (&[][..], &[][..]),
        (&[0][..], &[][..]),
        (&[0][..], &[0, 0][..]),
        (&[0xd8, 0, 0xff, 0xd8][..], &[0, 0xff][..]),
    ] {
        for search in [scalar as Search, bounded, reference] {
            assert_eq!(
                search(haystack, needle, &mut Meter::default()),
                Ok(oracle(haystack, needle))
            );
        }
    }
}

#[test]
fn front_matches_preserve_exact_existing_work_admission() {
    for length in [1, 2, 16, 32, 33, 512] {
        let haystack = vec![0xff; length + 100];
        let needle = vec![0xff; length];
        let (expected, old_work) = production(&haystack, &needle);
        for limit in 0..=old_work + 1 {
            let mut old = Meter {
                limit,
                ..Default::default()
            };
            let mut new = Meter {
                limit,
                ..Default::default()
            };
            assert_eq!(
                bounded(&haystack, &needle, &mut new),
                scalar(&haystack, &needle, &mut old)
            );
            assert_eq!((old.work, old.checkpoints), (new.work, new.checkpoints));
        }
        let mut meter = Meter::default();
        assert_eq!(scalar(&haystack, &needle, &mut meter), Ok(expected));
        assert_eq!(meter.work, old_work);
    }
}

#[test]
fn later_early_matches_expose_precharge_incompatibility_instead_of_hiding_it() {
    let mut haystack = vec![0; 8192];
    haystack[1] = 1;
    let needle = [1];
    let mut old = Meter::default();
    let mut new = Meter::default();
    assert_eq!(scalar(&haystack, &needle, &mut old), Ok(Some(1)));
    assert_eq!(bounded(&haystack, &needle, &mut new), Ok(Some(1)));
    assert_eq!(old.work, 5);
    assert_eq!(new.work, 4100);
    let mut limited = Meter {
        limit: 5,
        ..Default::default()
    };
    assert_eq!(
        bounded(&haystack, &needle, &mut limited),
        Err(Failure::Budget)
    );
}

#[test]
fn long_needles_retain_scalar_counts_and_failures() {
    let haystack = vec![b'a'; 160];
    let mut needle = vec![b'a'; 33];
    needle[32] = b'b';
    let mut full = Meter::default();
    assert_eq!(scalar(&haystack, &needle, &mut full), Ok(None));
    let (_, old_work) = production(&haystack, &needle);
    assert_eq!(full.work, old_work);
    for limit in [0, 1, 100, full.work - 1, full.work] {
        let mut old = Meter {
            limit,
            ..Default::default()
        };
        let mut new = Meter {
            limit,
            ..Default::default()
        };
        assert_eq!(
            bounded(&haystack, &needle, &mut new),
            scalar(&haystack, &needle, &mut old)
        );
        assert_eq!((new.work, new.checkpoints), (old.work, old.checkpoints));
    }
}

#[test]
fn every_bounded_checkpoint_can_cancel_before_the_next_search_span() {
    let haystack = vec![b'a'; 16_384];
    let mut needle = vec![b'a'; 32];
    needle[31] = b'b';
    let mut full = Meter::default();
    assert_eq!(bounded(&haystack, &needle, &mut full), Ok(None));
    for stop in 1..=full.checkpoints {
        let mut cancelled = Meter {
            cancel_at: Some(stop),
            ..Default::default()
        };
        assert_eq!(
            bounded(&haystack, &needle, &mut cancelled),
            Err(Failure::Cancelled)
        );
        assert_eq!(cancelled.checkpoints, stop);
    }
    for limit in [0, 1, 32, full.work - 1] {
        let mut limited = Meter {
            limit,
            ..Default::default()
        };
        assert_eq!(
            bounded(&haystack, &needle, &mut limited),
            Err(Failure::Budget)
        );
        assert!(limited.work <= limit);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x425954455345454b),
        ..Default::default()
    })]

    #[test]
    fn all_contenders_match_generated_arbitrary_bytes(
        haystack in prop::collection::vec(any::<u8>(), 0..20_000),
        needle in prop::collection::vec(any::<u8>(), 0..80),
    ) {
        let expected = oracle(&haystack, &needle);
        for search in [scalar as Search, bounded, reference] {
            prop_assert_eq!(search(&haystack, &needle, &mut Meter::default()), Ok(expected));
        }
        let mut meter = Meter::default();
        prop_assert_eq!(scalar(&haystack, &needle, &mut meter), Ok(expected));
        let (actual, work) = production(&haystack, &needle);
        prop_assert_eq!(actual, expected);
        prop_assert_eq!(meter.work, work);
    }
}
