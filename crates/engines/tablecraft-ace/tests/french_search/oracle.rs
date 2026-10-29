//! Exhaustive windows deliberately share neither production pruning nor bounds.
use super::{budget, order};
use proptest::prelude::*;
use std::{cmp::Ordering, ops::Range};
use tablecraft_ace::{FrenchTextSearch, compare_french_legacy_utf16, find_french_legacy_utf16};

fn boundaries(text: &[u16]) -> Vec<usize> {
    let mut result = vec![0];
    for character in char::decode_utf16(text.iter().copied()) {
        result.push(result.last().unwrap() + character.map_or(1, char::len_utf16));
    }
    result
}

pub(super) fn brute(
    text: &[u16],
    needle: &[u16],
    search: FrenchTextSearch,
) -> Option<Range<usize>> {
    windows(text, needle, search, |left, right| {
        compare_french_legacy_utf16(order(), left, right, &mut budget(), &|| false).unwrap()
            == Ordering::Equal
    })
}

fn windows(
    text: &[u16],
    needle: &[u16],
    search: FrenchTextSearch,
    equal: impl Fn(&[u16], &[u16]) -> bool,
) -> Option<Range<usize>> {
    let offsets = boundaries(text);
    let mut starts = offsets.clone();
    if matches!(search, FrenchTextSearch::ReverseThrough(_)) {
        starts.reverse();
    }
    for start in starts {
        let admitted = match search {
            FrenchTextSearch::ForwardFrom(first) => start >= first,
            FrenchTextSearch::ReverseThrough(last) => start <= last.min(text.len()),
        };
        if !admitted {
            continue;
        }
        if needle.is_empty() {
            return Some(start..start);
        }
        for end in offsets.iter().copied().filter(|end| *end > start) {
            if matches!(search, FrenchTextSearch::ReverseThrough(last) if end > last) {
                continue;
            }
            if equal(&text[start..end], needle) {
                return Some(start..end);
            }
        }
    }
    None
}

fn latin_identity(units: &[u16]) -> Vec<(u8, u8)> {
    // Independent closed Latin equality model, without native key tables or
    // production comparison. Accents remain distinct; case folds; NULL units
    // are ignorable; trailing ASCII spaces follow this engineering SQL policy.
    let mut end = units.len();
    while end > 0 && units[end - 1] == 32 {
        end -= 1;
    }
    units[..end]
        .iter()
        .filter_map(|unit| match unit {
            0 => None,
            32 => Some((b' ', 0)),
            65 | 97 => Some((b'a', 0)),
            69 | 101 => Some((b'e', 0)),
            0x00c8 | 0x00e8 => Some((b'e', 1)),
            0x00c9 | 0x00e9 => Some((b'e', 2)),
            _ => panic!("outside the independently declared Latin alphabet"),
        })
        .collect()
}

#[test]
fn exhaustive_closed_latin_windows_match_an_independent_literal_identity_model() {
    let alphabet = [0, 32, 65, 101, 0x00e8, 0x00e9];
    let mut texts = vec![vec![]];
    for first in alphabet {
        texts.push(vec![first]);
        for second in alphabet {
            texts.push(vec![first, second]);
            for third in alphabet {
                texts.push(vec![first, second, third]);
            }
        }
    }
    let needles = [
        vec![],
        vec![0],
        vec![97],
        vec![69],
        vec![0x00c8],
        vec![0x00c9],
        vec![101, 0x00e9],
        vec![0x00e9, 101],
        vec![101, 32],
    ];
    for text in texts {
        for needle in &needles {
            for bound in 0..=text.len() + 1 {
                for search in [
                    FrenchTextSearch::ForwardFrom(bound),
                    FrenchTextSearch::ReverseThrough(bound),
                ] {
                    let expected = windows(&text, needle, search, |left, right| {
                        latin_identity(left) == latin_identity(right)
                    });
                    let actual = find_french_legacy_utf16(
                        order(),
                        &text,
                        needle,
                        search,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(actual, expected, "{text:04x?}/{needle:04x?}/{search:?}");
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn generated_raw_windows_match_complete_french_comparison_without_pruning(
        text in prop::collection::vec(prop::sample::select(vec![0, 32, 65, 0x00e9, 0x0301, 0x00df,
            0x3005, 0x30fc, 0xd800, 0xdc00, 0xd83d, 0xde00]), 0..10),
        needle in prop::collection::vec(prop::sample::select(vec![0, 32, 97, 0x00c9, 0x0301, 0x00df,
            0x3005, 0x30fc, 0xd800, 0xdc00]), 0..5),
    ) {
        for bound in 0..=text.len() + 1 {
            for search in [FrenchTextSearch::ForwardFrom(bound), FrenchTextSearch::ReverseThrough(bound)] {
                let actual = find_french_legacy_utf16(order(), &text, &needle, search, &mut budget(), &|| false).unwrap();
                prop_assert_eq!(actual, brute(&text, &needle, search), "{:04x?}/{:04x?}/{:?}", text, needle, search);
            }
        }
    }
}
