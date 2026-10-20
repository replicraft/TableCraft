use super::*;
use sha2::{Digest, Sha256};
use std::cell::Cell;

fn order(family: u8) -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([9, 4, 0, family])
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 0, 1_000_000)
}
fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|part| u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
        .collect()
}
fn units(hex: &str) -> Vec<u16> {
    if hex == "-" {
        return Vec::new();
    }
    hex.as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|part| u16::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn independent_strings_cover_expansions_diacritics_kana_surrogates_overruns_and_truncation() {
    for (index, line) in include_str!("tests/vectors.tsv").lines().enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        let order = order(fields[0].parse().unwrap());
        let units = units(fields[1]);
        for (descending, expected) in [(false, fields[2]), (true, fields[3])] {
            let key =
                encode_utf16_text_index_key(order, &units, descending, &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(
                key.as_bytes(),
                bytes(expected),
                "vector {index}, descending {descending}"
            );
        }
    }
}
#[test]
fn every_utf16_unit_matches_independent_ascending_and_descending_key_digests() {
    for line in include_str!("tests/bmp.sha256").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        let order = order(fields[0].parse().unwrap());
        let first = u16::from_str_radix(fields[1], 16).unwrap() as u32 * 256;
        let mut hash = Sha256::new();
        for unit in first..first + 256 {
            for descending in [false, true] {
                let key = encode_utf16_text_index_key(
                    order,
                    &[unit as u16],
                    descending,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                hash.update((unit as u16).to_be_bytes());
                hash.update((key.as_bytes().len() as u16).to_be_bytes());
                hash.update(key.as_bytes());
            }
        }
        assert_eq!(
            hash.finalize().as_slice(),
            bytes(fields[2]),
            "family {}, block {}",
            fields[0],
            fields[1]
        );
    }
}
#[test]
fn text_wrapper_and_raw_surrogate_truncation_retain_exact_index_semantics() {
    for family in [0, 1] {
        for text in [
            "a",
            "é",
            "e\u{301}",
            "😀",
            "a  ",
            &format!("{}😀", "a".repeat(254)),
        ] {
            let units: Vec<_> = text.encode_utf16().collect();
            assert_eq!(
                encode_text_index_key(order(family), text, false, &mut budget(), &|| false)
                    .unwrap()
                    .as_bytes(),
                encode_utf16_text_index_key(order(family), &units, false, &mut budget(), &|| false)
                    .unwrap()
                    .as_bytes()
            );
        }
    }
}
#[test]
fn cancellation_and_resource_failures_publish_no_key_and_unknown_orders_do_not_fallback() {
    let units = vec![0x06d6; 255];
    let calls = Cell::new(0);
    encode_utf16_text_index_key(order(1), &units, false, &mut budget(), &|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let last = calls.get();
    for at in [1, 5, 10, last] {
        calls.set(0);
        assert!(matches!(
            encode_utf16_text_index_key(order(1), &units, false, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == at
            }),
            Err(InspectError::Cancelled)
        ));
    }
    assert!(matches!(
        encode_utf16_text_index_key(
            order(1),
            &units,
            false,
            &mut ResourceBudget::new(1, 0, 1000),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    for raw in [[0, 0, 0, 0], [9, 4, 1, 1], [9, 4, 0, 2]] {
        assert!(matches!(
            encode_utf16_text_index_key(
                NativeSortOrder::from_native_bytes(raw),
                &[],
                false,
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(512))]
    #[test]
    fn arbitrary_raw_utf16_sequences_are_bounded_and_preserve_descending_encoding(input in proptest::collection::vec(proptest::prelude::any::<u16>(), 0..600), family in 0u8..=1) {
        let ascending = encode_utf16_text_index_key(order(family), &input, false, &mut budget(), &|| false).unwrap();
        let descending = encode_utf16_text_index_key(order(family), &input, true, &mut budget(), &|| false).unwrap();
        let mut expected: Vec<_> = ascending.as_bytes().iter().map(|b| !b).collect();
        expected.push(0);
        proptest::prop_assert_eq!(descending.as_bytes(), expected);
    }
}
