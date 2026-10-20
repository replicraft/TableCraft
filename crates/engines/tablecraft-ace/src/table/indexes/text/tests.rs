use super::*;
use crate::table::indexes::key::TestKeyProfile;
use tablecraft_core::limits::ResourceBudget;

pub(super) fn profile(version: u8, descending: bool) -> TextProfile {
    TextProfile {
        column: 0,
        descending,
        order: NativeSortOrder::from_native_bytes([9, 4, 0, version]),
        maximum_units: Some(255),
        selection: EncodedSelection::All,
    }
}

#[test]
fn markers_and_reference_payloads_preserve_null_empty_case_and_accents() {
    // Fixed reference bytes from the separately run Jackcess encoder. The
    // expected framing/payloads do not use production constants or encoders.
    for (descending, null, empty, a, accent) in [
        (
            false,
            &[0][..],
            &[0x7f, 1, 0][..],
            &[0x7f, 0x4a, 1, 0][..],
            &[0x7f, 0x51, 1, 0x0e, 0][..],
        ),
        (
            true,
            &[0xff][..],
            &[0x80, 0xfe, 0xff, 0][..],
            &[0x80, 0xb5, 0xfe, 0xff, 0][..],
            &[0x80, 0xae, 0xfe, 0xf1, 0xff, 0][..],
        ),
    ] {
        let policy = profile(0, descending);
        for (value, golden) in [
            (Value::Null, null),
            (Value::Text(String::new()), empty),
            (Value::Text("A  ".into()), a),
            (Value::Text("é".into()), accent),
        ] {
            let key = policy.row_key_test(&[value]).unwrap();
            assert_eq!(key.0, golden);
            let (decoded, end) = policy.decode_test(golden).unwrap();
            assert!(key == decoded);
            assert_eq!(end, golden.len());
        }
        assert!(policy.decode_test(null).unwrap().0 != policy.decode_test(empty).unwrap().0);
        assert!(
            policy.row_key_test(&[Value::Text("a".into())]).unwrap()
                == policy.decode_test(a).unwrap().0
        );
    }
}

#[test]
fn framing_lengths_carriers_and_declared_utf16_width_fail_explicitly() {
    for descending in [false, true] {
        let policy = profile(0, descending);
        for bad in [
            &[][..],
            &[0x42][..],
            &[policy.null_marker(), 0][..],
            &[policy.value_marker()][..],
            &[policy.value_marker(), 0][..],
            &[policy.value_marker(), 2, 0][..],
        ] {
            assert!(matches!(
                policy.decode_test(bad),
                Err(InspectError::Malformed)
            ));
        }
        assert!(policy.row_key_test(&[Value::Int32(1)]).is_err());
        assert!(
            policy
                .row_key_test(&[Value::Text("a".repeat(256))])
                .is_err()
        );
        assert!(
            policy
                .row_key_test(&[Value::Text("😀".repeat(128))])
                .is_err()
        );
        assert!(
            policy
                .row_key_test(&[Value::Text("😀".repeat(127))])
                .is_ok()
        );
    }
}

#[test]
fn raw_key_copies_and_encoder_work_obey_shared_budget_and_cancellation() {
    let policy = profile(1, false);
    let mut no_bytes = ResourceBudget::new(0, 100, 100_000);
    assert!(
        policy
            .decode(
                &[0x7f, 1, 0],
                &mut KeyWork {
                    budget: &mut no_bytes,
                    cancelled: &|| false
                }
            )
            .is_err()
    );
    let mut no_steps = ResourceBudget::new(100_000, 100, 0);
    assert!(
        policy
            .row_key(
                &[Value::Text("a".into())],
                &mut KeyWork {
                    budget: &mut no_steps,
                    cancelled: &|| false
                }
            )
            .is_err()
    );
    let mut budget = ResourceBudget::new(100_000, 100, 100_000);
    assert!(matches!(
        policy.decode(
            &[0x7f, 1, 0],
            &mut KeyWork {
                budget: &mut budget,
                cancelled: &|| true
            }
        ),
        Err(InspectError::Cancelled)
    ));
    // Cancellation at the final copy check returns no owned key.
    let checks = std::cell::Cell::new(0);
    let cancel = || {
        checks.set(checks.get() + 1);
        checks.get() == 3
    };
    assert!(matches!(
        policy.decode(
            &[0x7f, 1, 0],
            &mut KeyWork {
                budget: &mut budget,
                cancelled: &cancel
            }
        ),
        Err(InspectError::Cancelled)
    ));
}

#[test]
fn overflow_digest_matches_independent_reference_and_preserves_kept_prefix() {
    for (length, seed, tail) in [
        (509, 0_u8, [0x47, 0x6c]),
        (509, 17_u8, [0x58, 0x7d]),
        (509, 255_u8, [0x46, 0x6b]),
        (510, 0_u8, [0x6c, 0x91]),
        (510, 17_u8, [0x7d, 0xa2]),
        (510, 255_u8, [0x6b, 0x90]),
        (511, 0_u8, [0xde, 0x90]),
        (511, 17_u8, [0xc9, 0xa3]),
        (511, 255_u8, [0xcc, 0x11]),
        (512, 0_u8, [0xbb, 0xdd]),
        (512, 17_u8, [0x26, 0xca]),
        (512, 255_u8, [0xbc, 0xcc]),
        (513, 0_u8, [0xce, 0xb9]),
        (513, 17_u8, [0xad, 0x24]),
        (513, 255_u8, [0x57, 0xbe]),
        (1024, 0_u8, [0x36, 0x84]),
        (1024, 17_u8, [0x56, 0x75]),
        (1024, 255_u8, [0x71, 0x6e]),
        (2048, 0_u8, [0x07, 0x62]),
        (2048, 17_u8, [0x99, 0x08]),
        (2048, 255_u8, [0x4a, 0x65]),
        (4096, 0_u8, [0x30, 0x81]),
        (4096, 17_u8, [0x78, 0x54]),
        (4096, 255_u8, [0xfa, 0xd6]),
    ] {
        let raw: Vec<u8> = (0..length)
            .map(|i| (i as u8).wrapping_mul(37).wrapping_add(seed))
            .collect();
        let mut encoded = raw.clone();
        let mut budget = ResourceBudget::new(100_000, 100, 100_000);
        digest::truncate(
            &mut encoded,
            &mut KeyWork {
                budget: &mut budget,
                cancelled: &|| false,
            },
        )
        .unwrap();
        assert_eq!(encoded.len(), length.min(510));
        assert_eq!(&encoded[encoded.len() - 2..], tail);
        if length <= 510 {
            assert_eq!(encoded, raw);
        } else {
            assert_eq!(&encoded[..508], &raw[..508]);
        }
    }
}

#[test]
fn digested_text_keys_match_rows_without_implying_exact_unique_comparison() {
    for descending in [false, true] {
        let policy = profile(1, descending);
        let key = policy
            .row_key_test(&[Value::Text("a".repeat(255))])
            .unwrap();
        assert_eq!(key.0.len(), 510);
        assert!(!policy.unique_key_is_exact(&key));
        assert!(policy.decode_test(&key.0).unwrap().0 == key);
        let short = policy.row_key_test(&[Value::Text("abc".into())]).unwrap();
        assert!(policy.unique_key_is_exact(&short));
    }
}

#[test]
fn memo_index_prefix_does_not_truncate_returned_values_or_prove_uniqueness() {
    let mut memo = profile(0, false);
    memo.maximum_units = None;
    let prefix = "a".repeat(255);
    let left = Value::Text(prefix.clone() + &"x".repeat(4096));
    let right = Value::Text(prefix + "different suffix");
    let key = memo.row_key_test(std::slice::from_ref(&left)).unwrap();
    let mut expected = vec![0x7f];
    expected.extend(std::iter::repeat_n(0x4a, 255));
    expected.extend([1, 0]);
    assert_eq!(key.0, expected);
    assert!(memo.row_key_test(std::slice::from_ref(&right)).unwrap() == key);
    assert!(!memo.unique_key_is_exact(&key));
    assert_ne!(left, right);
    assert!(profile(0, false).row_key_test(&[left]).is_err());

    // Trim is applied to the retained prefix, not to the entire Memo value.
    let spaced = Value::Text("a".repeat(254) + " Z");
    let trimmed = memo.row_key_test(&[spaced]).unwrap();
    let mut expected = vec![0x7f];
    expected.extend(std::iter::repeat_n(0x4a, 254));
    expected.extend([1, 0]);
    assert_eq!(trimmed.0, expected);
}
