use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_ace::{InspectError, NativeSortOrder, compare_general_text, encode_text_index_key};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
}
fn order(version: u8) -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([9, 4, 0, version])
}
fn compare(version: u8, left: &str, right: &str) -> Ordering {
    compare_general_text(order(version), left, right, &mut budget(), &|| false).unwrap()
}

#[test]
fn explicit_general_weights_cover_case_accents_expansions_and_trailing_spaces() {
    for version in [0, 1] {
        for (left, right) in [("a", "A"), ("a", "A  "), ("é", "e\u{301}"), ("ß", "ss")] {
            assert_eq!(compare(version, left, right), Ordering::Equal);
        }
        for (left, right) in [("a", "b"), ("e", "é"), ("", "a")] {
            assert_eq!(compare(version, left, right), Ordering::Less);
            assert_eq!(compare(version, right, left), Ordering::Greater);
        }
    }
}

#[test]
fn long_values_retain_suffixes_and_supplementary_pairs_beyond_native_key_prefixes() {
    for version in [0, 1] {
        for length in [254, 255, 1024, 20_000] {
            let prefix = "x".repeat(length);
            let left = format!("{prefix}a");
            let right = format!("{prefix}b");
            assert_eq!(compare(version, &left, &right), Ordering::Less);
            assert_eq!(
                compare(version, &left, &format!("{prefix}A  ")),
                Ordering::Equal
            );
            if length >= 255 {
                assert_eq!(
                    encode_text_index_key(order(version), &left, false, &mut budget(), &|| false)
                        .unwrap()
                        .as_bytes(),
                    encode_text_index_key(order(version), &right, false, &mut budget(), &|| false)
                        .unwrap()
                        .as_bytes()
                );
            }
            assert_eq!(
                compare(version, &format!("{prefix}😀a"), &format!("{prefix}😀b")),
                Ordering::Less
            );
            // Late unprintables use wide positions; they never wrap native
            // 16-bit offsets or alias earlier occurrences in a long string.
            assert_ne!(
                compare(version, &format!("{prefix}a-b"), &format!("{prefix}ab")),
                Ordering::Equal
            );
        }
    }
}

#[test]
fn short_comparisons_agree_with_independent_complete_key_vectors() {
    let mut cases = Vec::new();
    for line in include_str!("../src/collation/key/tests/vectors.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        let units: Vec<_> = if fields[1] == "-" {
            Vec::new()
        } else {
            fields[1]
                .as_bytes()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|part| u16::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
                .collect()
        };
        if units.len() > 255 {
            continue;
        }
        let Ok(text) = String::from_utf16(&units) else {
            continue;
        };
        let key: Vec<_> = fields[2]
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|part| u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
            .collect();
        cases.push((fields[0].parse::<u8>().unwrap(), text, key));
    }
    // Pair every independent vector with its peers; a widened suffix must not
    // change short-value equality or order. These are index-model observations,
    // not evidence for operation-specific native SQL Long Text behavior.
    for (version, left, key) in &cases {
        for (_, right, other) in cases.iter().filter(|(v, _, _)| v == version) {
            assert_eq!(
                compare(*version, left, right),
                key.cmp(other),
                "version {version}: {left:?} / {right:?}"
            );
        }
    }
}

#[test]
fn limits_unknown_descriptors_and_every_cancellation_checkpoint_fail_explicitly() {
    for raw in [[0, 0, 0, 0], [9, 4, 1, 1], [9, 4, 0, 2]] {
        assert!(matches!(
            compare_general_text(
                NativeSortOrder::from_native_bytes(raw),
                "a",
                "b",
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
    for mut limits in [
        ResourceBudget::new(0, 0, 1000),
        ResourceBudget::new(1_000_000, 0, 1),
    ] {
        assert!(matches!(
            compare_general_text(order(1), "a", "b", &mut limits, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
    let visits = Cell::new(0);
    let probe = || {
        visits.set(visits.get() + 1);
        false
    };
    compare_general_text(order(1), "a-béカ", "A'bEか", &mut budget(), &probe).unwrap();
    let total = visits.get();
    for at in 1..=total {
        visits.set(0);
        assert!(
            matches!(
                compare_general_text(order(1), "a-béカ", "A'bEか", &mut budget(), &|| {
                    visits.set(visits.get() + 1);
                    visits.get() == at
                }),
                Err(InspectError::Cancelled)
            ),
            "checkpoint {at}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn ordering_laws_hold_for_complete_unicode_strings(a in ".{0,25}", b in ".{0,25}", c in ".{0,25}") {
        for version in [0, 1] {
            prop_assert_eq!(compare(version, &a, &a), Ordering::Equal);
            let ab = compare(version, &a, &b);
            prop_assert_eq!(ab, compare(version, &b, &a).reverse());
            if ab != Ordering::Greater && compare(version, &b, &c) != Ordering::Greater {
                prop_assert_ne!(compare(version, &a, &c), Ordering::Greater);
            }
        }
    }
}
