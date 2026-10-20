use super::*;
use proptest::prelude::*;
#[path = "selection_tests.rs"]
mod selection;

fn profile(descending: bool) -> BinaryProfile<'static> {
    BinaryProfile {
        column: 0,
        descending,
        maximum: MAX_SHORT_BINARY_BYTES,
        selection: BinaryIndexSelection::All,
    }
}
// Independent engineering encoding from the reviewed segment format, not a
// production encoder/decode round trip. Decoded keys are compared with raw bytes.
fn encoded(bytes: &[u8], descending: bool) -> Vec<u8> {
    let mut result = vec![if descending { DESC_VALUE } else { ASC_VALUE }];
    let count = bytes.len().div_ceil(SEGMENT_BYTES);
    for (i, chunk) in bytes.chunks(SEGMENT_BYTES).enumerate() {
        let mut block = [0_u8; ENCODED_SEGMENT_BYTES];
        block[..chunk.len()].copy_from_slice(chunk);
        let continued = i + 1 < count;
        block[SEGMENT_BYTES] = if continued {
            CONTINUED_SEGMENT
        } else {
            chunk.len() as u8
        };
        if descending {
            for byte in &mut block {
                *byte = !*byte;
            }
            if continued {
                block[SEGMENT_BYTES] = CONTINUED_SEGMENT;
            }
        }
        result.extend(block);
    }
    result
}

#[test]
fn null_empty_partial_full_and_continued_goldens_have_exact_consumption() {
    let asc = profile(false);
    assert!(asc.decode(&[ASC_NULL]).unwrap().0.is_none());
    assert_eq!(asc.decode(&[ASC_VALUE]).unwrap().0.unwrap().bytes(), &[]);
    let short = [ASC_VALUE, 1, 2, 3, 0, 0, 0, 0, 0, 3];
    let (key, end) = asc.decode(&short).unwrap();
    assert_eq!(end, short.len());
    assert_eq!(key.unwrap().bytes(), &[1, 2, 3]);
    let desc = [
        DESC_VALUE, 0xfe, 0xfd, 0xfc, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfc,
    ];
    assert_eq!(
        profile(true).decode(&desc).unwrap().0.unwrap().bytes(),
        &[1, 2, 3]
    );
    for len in [1, 7, 8, 9, 15, 16, 17, 254, 255, 509, 510] {
        let bytes = (0..len).map(|i| (i * 31) as u8).collect::<Vec<_>>();
        for descending in [false, true] {
            let data = encoded(&bytes, descending);
            let (key, end) = profile(descending).decode(&data).unwrap();
            assert_eq!(key.unwrap().bytes(), bytes);
            assert_eq!(end, data.len());
        }
    }
    for descending in [false, true] {
        let policy = profile(descending);
        let empty = policy.decode(&encoded(&[], descending)).unwrap().0;
        let nonempty = policy.decode(&encoded(&[0], descending)).unwrap().0;
        assert!(policy.compare(&empty, &nonempty).is_lt());
    }
    assert!(
        asc.compare(&None, &asc.decode(&[ASC_VALUE]).unwrap().0)
            .is_lt()
    );
}

#[test]
fn invalid_markers_padding_lengths_continuation_and_declared_widths_fail() {
    for descending in [false, true] {
        let policy = profile(descending);
        assert!(policy.decode(&[]).is_err());
        assert!(policy.decode(&[42]).is_err());
        let data = encoded(&[1], descending);
        for cut in 2..data.len() {
            assert!(policy.decode(&data[..cut]).is_err());
        }
        let mut padding = data.clone();
        padding[2] ^= 1;
        assert!(policy.decode(&padding).is_err());
        for length in [0, 10, 255] {
            let mut bad = data.clone();
            bad[ENCODED_SEGMENT_BYTES] = if descending { !length } else { length };
            assert!(policy.decode(&bad).is_err());
        }
        let mut dangling = encoded(&[1; 8], descending);
        dangling[ENCODED_SEGMENT_BYTES] = CONTINUED_SEGMENT;
        assert!(policy.decode(&dangling).is_err());
        let limited = BinaryProfile {
            maximum: 8,
            ..policy
        };
        assert!(limited.decode(&encoded(&[1; 9], descending)).is_err());
        assert!(limited.row_key(&[Value::Binary(vec![1; 9])]).is_err());
        assert!(limited.row_key(&[Value::Int32(1)]).is_err());
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config { cases: 128, rng_seed: proptest::test_runner::RngSeed::Fixed(0x4249_4e41), ..Default::default() })]
    #[test]
    fn generated_binary_keys_preserve_raw_lexicographic_order_and_exact_rows(left in proptest::collection::vec(any::<u8>(),0..=MAX_SHORT_BINARY_BYTES), right in proptest::collection::vec(any::<u8>(),0..=MAX_SHORT_BINARY_BYTES), descending in any::<bool>()) {
        let policy=profile(descending);
        let l=policy.decode(&encoded(&left,descending)).unwrap().0;
        let r=policy.decode(&encoded(&right,descending)).unwrap().0;
        let decoded_left=l.unwrap();
        prop_assert_eq!(decoded_left.bytes(),left.as_slice());
        prop_assert!(l == policy.row_key(&[Value::Binary(left.clone())]).unwrap());
        let expected=left.cmp(&right);
        let expected=if left.is_empty() || right.is_empty() { left.len().cmp(&right.len()) } else if descending { expected.reverse() } else { expected };
        prop_assert_eq!(policy.compare(&l,&r),expected);
    }
}
