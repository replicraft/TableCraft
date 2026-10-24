#[path = "general_search/raw.rs"]
mod raw;

use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering, ops::Range};
use tablecraft_ace::{
    GeneralTextSearch, InspectError, NativeSortOrder, compare_general_text, find_general_text,
};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
}
fn order(version: u8) -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([9, 4, 0, version])
}
fn find(version: u8, text: &str, needle: &str, search: GeneralTextSearch) -> Option<Range<usize>> {
    find_general_text(order(version), text, needle, search, &mut budget(), &|| {
        false
    })
    .unwrap()
}

#[test]
fn expansions_case_accents_and_reverse_end_bounds_return_consumed_utf16_spans() {
    for version in [0, 1] {
        for (text, needle, search, expected) in [
            (
                "Straße STRASSE",
                "ss",
                GeneralTextSearch::ForwardFrom(0),
                Some(4..5),
            ),
            (
                "Straße STRASSE",
                "ss",
                GeneralTextSearch::ForwardFrom(5),
                Some(11..13),
            ),
            (
                "Straße STRASSE",
                "ss",
                GeneralTextSearch::ReverseThrough(14),
                Some(11..13),
            ),
            (
                "Straße STRASSE",
                "ss",
                GeneralTextSearch::ReverseThrough(12),
                Some(4..5),
            ),
            ("SS ß", "ß", GeneralTextSearch::ForwardFrom(0), Some(0..2)),
            (
                "SS ß",
                "ß",
                GeneralTextSearch::ReverseThrough(4),
                Some(3..4),
            ),
            ("😀éXé", "É", GeneralTextSearch::ForwardFrom(1), Some(2..3)),
            ("😀éXé", "É", GeneralTextSearch::ReverseThrough(1), None),
            ("a ", "a ", GeneralTextSearch::ForwardFrom(0), Some(0..1)),
            ("\0a", "a", GeneralTextSearch::ForwardFrom(0), Some(0..2)),
            ("\0a", "a", GeneralTextSearch::ReverseThrough(2), Some(1..2)),
            ("a\0", "\0", GeneralTextSearch::ForwardFrom(0), Some(1..2)),
            ("a", "\0", GeneralTextSearch::ForwardFrom(0), None),
        ] {
            assert_eq!(
                find(version, text, needle, search),
                expected,
                "General {version}: {text:?} / {needle:?}, {search:?}"
            );
        }
        // The independent legacy key vector ignores this supplementary pair;
        // General 2010 retains its primary weights. Earliest spans follow that
        // model difference instead of assuming both families count it alike.
        assert_eq!(
            find(version, "😀éÉ", "é", GeneralTextSearch::ForwardFrom(0)),
            Some(if version == 0 { 0..4 } else { 2..4 })
        );
    }
}

#[test]
fn empty_needles_empty_inputs_and_surrogate_boundaries_are_explicit() {
    for version in [0, 1] {
        for (text, needle, search, expected) in [
            ("", "", GeneralTextSearch::ForwardFrom(0), Some(0..0)),
            ("", "a", GeneralTextSearch::ForwardFrom(0), None),
            ("😀a", "", GeneralTextSearch::ForwardFrom(1), Some(2..2)),
            ("😀a", "", GeneralTextSearch::ReverseThrough(1), Some(0..0)),
            ("😀a", "", GeneralTextSearch::ForwardFrom(4), None),
            ("😀a", "", GeneralTextSearch::ReverseThrough(4), Some(3..3)),
        ] {
            assert_eq!(find(version, text, needle, search), expected);
        }
    }
}

#[test]
fn long_matching_needles_do_not_require_encoding_every_shorter_prefix() {
    for version in [0, 1] {
        let needle = "x".repeat(1024) + "a";
        let text = "x".repeat(1024) + "A";
        assert_eq!(
            find_general_text(
                order(version),
                &text,
                &needle,
                GeneralTextSearch::ForwardFrom(0),
                &mut ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
                &|| false
            )
            .unwrap(),
            Some(0..1025)
        );
        assert_eq!(
            find(
                version,
                &text,
                &("x".repeat(1024) + "b"),
                GeneralTextSearch::ForwardFrom(0)
            ),
            None
        );
    }
}

#[test]
fn unknown_orders_limits_and_every_measured_cancellation_point_fail_explicitly() {
    assert!(matches!(
        find_general_text(
            NativeSortOrder::from_native_bytes([9, 4, 0, 2]),
            "a",
            "a",
            GeneralTextSearch::ForwardFrom(0),
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    for mut budget in [
        ResourceBudget::new(0, 0, 1000),
        ResourceBudget::new(1_000_000, 0, 1),
    ] {
        assert!(matches!(
            find_general_text(
                order(1),
                "Straße",
                "ss",
                GeneralTextSearch::ForwardFrom(0),
                &mut budget,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    for search in [
        GeneralTextSearch::ForwardFrom(0),
        GeneralTextSearch::ReverseThrough(6),
    ] {
        let calls = Cell::new(0);
        find_general_text(order(1), "Straße", "ss", search, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        let total = calls.get();
        for stop in 1..=total {
            calls.set(0);
            assert!(
                matches!(
                    find_general_text(order(1), "Straße", "ss", search, &mut budget(), &|| {
                        calls.set(calls.get() + 1);
                        calls.get() == stop
                    }),
                    Err(InspectError::Cancelled)
                ),
                "checkpoint {stop}"
            );
        }
    }
}

fn brute(version: u8, text: &str, needle: &str, search: GeneralTextSearch) -> Option<Range<usize>> {
    let mut offsets: Vec<_> = text.char_indices().map(|(byte, _)| byte).collect();
    offsets.push(text.len());
    let reverse = matches!(search, GeneralTextSearch::ReverseThrough(_));
    let mut starts = offsets.clone();
    if reverse {
        starts.reverse();
    }
    for start in starts {
        let begin = text[..start].encode_utf16().count();
        let allowed = match search {
            GeneralTextSearch::ForwardFrom(first) => begin >= first,
            GeneralTextSearch::ReverseThrough(last) => {
                begin <= last.min(text.encode_utf16().count())
            }
        };
        if !allowed {
            continue;
        }
        if needle.is_empty() {
            return Some(begin..begin);
        }
        for end in offsets.iter().copied().filter(|end| *end > start) {
            let finish = text[..end].encode_utf16().count();
            if matches!(search, GeneralTextSearch::ReverseThrough(last) if finish>last) {
                continue;
            }
            if compare_general_text(
                order(version),
                &text[start..end],
                needle,
                &mut budget(),
                &|| false,
            )
            .unwrap()
                == Ordering::Equal
            {
                return Some(begin..finish);
            }
        }
    }
    None
}

fn strings(maximum: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop::sample::select(vec![
            'a', 'A', 'b', 'e', 'é', '\u{301}', 'ß', 's', ' ', 'カ', 'ー', '々', '\0', '😀',
        ]),
        0..=maximum,
    )
    .prop_map(|characters| characters.into_iter().collect())
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn primary_pruning_and_lower_bound_probing_agree_with_unpruned_full_span_enumeration(text in strings(7), needle in strings(3)) {
        for version in [0,1] {
            for bound in 0..=text.encode_utf16().count()+1 {
                for search in [GeneralTextSearch::ForwardFrom(bound),GeneralTextSearch::ReverseThrough(bound)] {
                    prop_assert_eq!(find(version,&text,&needle,search), brute(version,&text,&needle,search), "{:?} / {:?}, {:?}",text,needle,search);
                }
            }
        }
    }
}
