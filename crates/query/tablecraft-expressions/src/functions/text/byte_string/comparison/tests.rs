use super::*;
use crate::functions::text::byte_string::ByteString;
use proptest::prelude::*;

fn raw(units: &[u16]) -> ByteOperand {
    let bytes = units.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    ByteOperand::Bytes(ByteString::from_owned(bytes).unwrap())
}

#[test]
fn complete_units_are_required_even_after_an_early_differing_prefix() {
    let odd = ByteOperand::Bytes(ByteString::from_owned(vec![0, 0, 65]).unwrap());
    assert_eq!(
        compare_binary_utf16(&raw(&[1]), &odd, &mut || Ok(())),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        compare_binary_utf16(
            &raw(&[0xd83d, 0xde00]),
            &ByteOperand::Scalar(Value::Text("😀".into())),
            &mut || Ok(())
        ),
        Ok(Ordering::Equal)
    );
}

#[test]
fn binary_unit_scan_propagates_each_checkpoint_failure() {
    let left = raw(&[0xd800, 0, 65]);
    let right = raw(&[0xd800, 0, 65]);
    for stop in 1..=4 {
        let mut calls = 0;
        assert_eq!(
            compare_binary_utf16(&left, &right, &mut || {
                calls += 1;
                if calls == stop {
                    Err(EvalError::Cancelled)
                } else {
                    Ok(())
                }
            }),
            Err(EvalError::Cancelled)
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5354_5243_4f4d_5052),
        failure_persistence: None,
        ..Default::default()
    })]

    #[test]
    fn raw_little_endian_ranges_follow_independent_unsigned_unit_order(
        left in proptest::collection::vec(any::<u16>(), 0..24),
        right in proptest::collection::vec(any::<u16>(), 0..24),
    ) {
        prop_assert_eq!(
            compare_binary_utf16(&raw(&left), &raw(&right), &mut || Ok(())),
            Ok(left.cmp(&right))
        );
    }
}
