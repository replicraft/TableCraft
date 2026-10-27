use std::{cell::Cell, cmp::Ordering};
use tablecraft_ace::{
    InspectError, NativeSortOrder, compare_french_legacy_text, compare_french_legacy_utf16,
    encode_text_index_key,
};
use tablecraft_core::limits::ResourceBudget;

fn order() -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([12, 4, 0, 0])
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
}

fn compare(left: &str, right: &str) -> Ordering {
    compare_french_legacy_text(order(), left, right, &mut budget(), &|| false).unwrap()
}

fn hex_bytes(value: &str) -> Vec<u8> {
    if value == "-" {
        return Vec::new();
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn complete_short_keys_agree_with_independent_french_encoder() {
    let mut cases = Vec::new();
    for line in include_str!("french_legacy_keys/vectors.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        let bytes = hex_bytes(fields[0]);
        let units: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect();
        if units.len() <= 255 {
            cases.push((units, hex_bytes(fields[1])));
        }
    }
    assert!(cases.len() > 200);
    for pair in cases.windows(2) {
        let (left, left_key) = &pair[0];
        let (right, right_key) = &pair[1];
        let expected = left_key.cmp(right_key);
        assert_eq!(
            compare_french_legacy_utf16(order(), left, right, &mut budget(), &|| false).unwrap(),
            expected
        );
        assert_eq!(
            compare_french_legacy_utf16(order(), right, left, &mut budget(), &|| false).unwrap(),
            expected.reverse()
        );
    }
}

#[test]
fn long_latin_controls_match_independent_weight_stream_model() {
    // These literal signs were independently computed by latin_model.py from
    // measured Jackcess CharHandler metadata and 158 complete short-key checks.
    // This is long-Latin engineering evidence, not native SQL/NLS qualification.
    for length in [256, 257, 1024, 20_000] {
        let middle = "a".repeat(length);
        let cases = [
            (format!("{middle}b"), format!("{middle}c"), Ordering::Less),
            (
                format!("é{middle}è"),
                format!("è{middle}é"),
                Ordering::Greater,
            ),
            (format!("é{middle}e"), format!("e{middle}é"), Ordering::Less),
            (
                format!("é{middle}"),
                format!("é{middle}  "),
                Ordering::Equal,
            ),
            (
                format!("é{middle}"),
                format!("É{}", "A".repeat(length)),
                Ordering::Equal,
            ),
        ];
        for (left, right, expected) in cases {
            assert_eq!(compare(&left, &right), expected);
            assert_eq!(compare(&right, &left), expected.reverse());
        }
        let left = format!("{middle}b");
        let right = format!("{middle}c");
        assert_eq!(
            encode_text_index_key(order(), &left, false, &mut budget(), &|| false)
                .unwrap()
                .as_bytes(),
            encode_text_index_key(order(), &right, false, &mut budget(), &|| false)
                .unwrap()
                .as_bytes()
        );
    }
}

#[test]
fn full_position_extension_and_raw_units_remain_separate_from_long_latin_oracle() {
    for length in [255, 1024, 20_000] {
        let prefix = "a".repeat(length);
        assert_ne!(
            compare(&format!("{prefix}a-b"), &format!("{prefix}ab")),
            Ordering::Equal
        );
        let mut left: Vec<_> = prefix.encode_utf16().collect();
        left.extend([0xd800, b'b' as u16]);
        let mut right = left.clone();
        *right.last_mut().unwrap() = b'c' as u16;
        assert_eq!(
            compare_french_legacy_utf16(order(), &left, &right, &mut budget(), &|| false).unwrap(),
            Ordering::Less
        );
    }
    assert_eq!(compare("cote", "COTE  "), Ordering::Equal);
    assert_ne!(compare("cote", "cote\u{a0}"), Ordering::Equal);
}

#[test]
fn exact_descriptor_cumulative_limits_and_every_cancellation_checkpoint_are_checked() {
    for raw in [[9, 4, 0, 0], [12, 4, 1, 0], [12, 4, 0, 1], [0, 0, 0, 0]] {
        assert!(matches!(
            compare_french_legacy_text(
                NativeSortOrder::from_native_bytes(raw),
                "a",
                "b",
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
    for mut limited in [
        ResourceBudget::new(1, 0, 100_000),
        ResourceBudget::new(1_000_000, 0, 1),
    ] {
        assert!(matches!(
            compare_french_legacy_text(order(), "é", "è", &mut limited, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
    // An early unequal primary must not bypass work/errors in the other input.
    // The short operand fits this pool; the complete long operand cannot fit.
    let long = "z".repeat(20_000);
    for (left, right) in [("a", long.as_str()), (long.as_str(), "a")] {
        let mut limited = ResourceBudget::new(1024, 0, 1_000_000);
        assert!(matches!(
            compare_french_legacy_text(order(), left, right, &mut limited, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    compare_french_legacy_text(order(), "éabc", "èabd", &mut budget(), &observe).unwrap();
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancelled = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        assert!(matches!(
            compare_french_legacy_text(order(), "éabc", "èabd", &mut budget(), &cancelled),
            Err(InspectError::Cancelled)
        ));
    }
    assert_eq!(compare("éabc", "èabd"), Ordering::Less);
}
