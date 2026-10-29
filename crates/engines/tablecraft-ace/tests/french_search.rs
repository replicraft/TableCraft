mod support {
    pub fn order() -> tablecraft_ace::NativeSortOrder {
        tablecraft_ace::NativeSortOrder::from_native_bytes([12, 4, 0, 0])
    }

    pub fn budget() -> tablecraft_core::limits::ResourceBudget {
        tablecraft_core::limits::ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
    }
}

#[path = "french_search/oracle.rs"]
mod oracle;

use std::{cell::Cell, ops::Range};
use support::{budget, order};
use tablecraft_ace::{
    FrenchTextSearch, InspectError, NativeSortOrder, find_french_legacy_text,
    find_french_legacy_utf16,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn find(text: &str, needle: &str, search: FrenchTextSearch) -> Option<Range<usize>> {
    find_french_legacy_text(order(), text, needle, search, &mut budget(), &|| false).unwrap()
}

#[test]
fn accents_case_padding_expansions_and_reverse_bounds_return_original_spans() {
    for (text, needle, search, expected) in [
        (
            "cote côte",
            "côte",
            FrenchTextSearch::ForwardFrom(0),
            Some(5..9),
        ),
        (
            "côte CÔTE",
            "côte",
            FrenchTextSearch::ReverseThrough(9),
            Some(5..9),
        ),
        (
            "côte CÔTE",
            "côte",
            FrenchTextSearch::ReverseThrough(8),
            Some(0..4),
        ),
        (
            "côté côte",
            "côte",
            FrenchTextSearch::ForwardFrom(0),
            Some(5..9),
        ),
        ("éè èé", "èé", FrenchTextSearch::ForwardFrom(0), Some(3..5)),
        (
            "éè èé",
            "éè",
            FrenchTextSearch::ReverseThrough(5),
            Some(0..2),
        ),
        ("a  ", "A ", FrenchTextSearch::ForwardFrom(0), Some(0..1)),
        ("SS ß", "ß", FrenchTextSearch::ForwardFrom(0), Some(0..2)),
        (
            "SS ß",
            "ss",
            FrenchTextSearch::ReverseThrough(4),
            Some(3..4),
        ),
        ("\0a", "a", FrenchTextSearch::ForwardFrom(0), Some(0..2)),
        ("a\0", "\0", FrenchTextSearch::ForwardFrom(0), Some(1..2)),
        ("a", "\0", FrenchTextSearch::ForwardFrom(0), None),
        ("a", "", FrenchTextSearch::ForwardFrom(2), None),
        ("a", "", FrenchTextSearch::ReverseThrough(99), Some(1..1)),
    ] {
        assert_eq!(
            find(text, needle, search),
            expected,
            "{text:?}/{needle:?}/{search:?}"
        );
    }
}

#[test]
fn raw_units_keep_isolated_surrogates_and_never_split_valid_pairs() {
    let text = [0xd83d, 0xde00, 0xd800, 0x00e9, 0xdc00];
    for needle in [vec![0xd800], vec![0xdc00], vec![0x00c9], vec![]] {
        for bound in 0..=text.len() + 1 {
            for search in [
                FrenchTextSearch::ForwardFrom(bound),
                FrenchTextSearch::ReverseThrough(bound),
            ] {
                assert_eq!(
                    find_french_legacy_utf16(
                        order(),
                        &text,
                        &needle,
                        search,
                        &mut budget(),
                        &|| false
                    )
                    .unwrap(),
                    oracle::brute(&text, &needle, search)
                );
            }
        }
    }
    assert_eq!(
        find("😀a", "", FrenchTextSearch::ForwardFrom(1)),
        Some(2..2)
    );
    assert_eq!(
        find("😀a", "", FrenchTextSearch::ReverseThrough(1)),
        Some(0..0)
    );
}

#[test]
fn long_search_avoids_exhaustive_prefix_encoding_and_mismatch_work_scales_linearly() {
    let needle = "x".repeat(2048) + "é";
    let text = "x".repeat(2048) + "É";
    assert_eq!(
        find_french_legacy_text(
            order(),
            &text,
            &needle,
            FrenchTextSearch::ForwardFrom(0),
            &mut ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
            &|| false
        )
        .unwrap(),
        Some(0..2049)
    );
    assert_eq!(
        find(
            &text,
            &("x".repeat(2048) + "è"),
            FrenchTextSearch::ForwardFrom(0)
        ),
        None
    );

    let mut measured = Vec::new();
    for repetitions in [128, 256] {
        let text = "Les élèves lisent le livre. ".repeat(repetitions);
        let mut pool = ResourceBudget::new(32 * 1024 * 1024, 0, 1_000_000);
        assert_eq!(
            find_french_legacy_text(
                order(),
                &text,
                "zèbre",
                FrenchTextSearch::ForwardFrom(0),
                &mut pool,
                &|| false
            )
            .unwrap(),
            None
        );
        measured.push(1_000_000 - pool.remaining(Resource::Steps));
    }
    assert!(measured[1] <= 3 * measured[0], "work counts {measured:?}");
}

#[test]
fn source_identity_cumulative_limits_and_every_cancellation_checkpoint_are_enforced() {
    for raw in [[9, 4, 0, 0], [12, 4, 1, 0], [12, 4, 0, 1]] {
        assert!(matches!(
            find_french_legacy_text(
                NativeSortOrder::from_native_bytes(raw),
                "",
                "",
                FrenchTextSearch::ForwardFrom(0),
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
    for pool in [
        ResourceBudget::new(0, 0, 100_000),
        ResourceBudget::new(1_000_000, 0, 1),
    ] {
        let mut pool = pool;
        assert!(matches!(
            find_french_legacy_text(
                order(),
                "côte",
                "ôte",
                FrenchTextSearch::ForwardFrom(0),
                &mut pool,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    for search in [
        FrenchTextSearch::ForwardFrom(0),
        FrenchTextSearch::ReverseThrough(4),
    ] {
        let calls = Cell::new(0);
        find_french_legacy_text(order(), "côte", "ôte", search, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        let total = calls.get();
        for stop in 1..=total {
            calls.set(0);
            assert!(
                matches!(
                    find_french_legacy_text(
                        order(),
                        "côte",
                        "ôte",
                        search,
                        &mut budget(),
                        &|| {
                            calls.set(calls.get() + 1);
                            calls.get() == stop
                        }
                    ),
                    Err(InspectError::Cancelled)
                ),
                "checkpoint {stop}/{total}"
            );
        }
    }
}

#[test]
fn raw_search_charges_exact_allocations_and_work_without_resetting_the_callers_pool() {
    let text = [0xd800, 0x00e9, 0x0301, 97, 0x00c9];
    let needle = [0x00c9];
    let mut measured = budget();
    let before_bytes = measured.remaining(Resource::Bytes);
    let before_steps = measured.remaining(Resource::Steps);
    find_french_legacy_utf16(
        order(),
        &text,
        &needle,
        FrenchTextSearch::ForwardFrom(0),
        &mut measured,
        &|| false,
    )
    .unwrap();
    let bytes = before_bytes - measured.remaining(Resource::Bytes);
    let steps = before_steps - measured.remaining(Resource::Steps);
    for pool in [
        ResourceBudget::new(bytes - 1, 0, steps),
        ResourceBudget::new(bytes, 0, steps - 1),
    ] {
        let mut pool = pool;
        assert!(matches!(
            find_french_legacy_utf16(
                order(),
                &text,
                &needle,
                FrenchTextSearch::ForwardFrom(0),
                &mut pool,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    let mut pool = ResourceBudget::new(bytes, 0, steps);
    assert!(
        find_french_legacy_utf16(
            order(),
            &text,
            &needle,
            FrenchTextSearch::ForwardFrom(0),
            &mut pool,
            &|| false
        )
        .is_ok()
    );
    assert!(matches!(
        find_french_legacy_utf16(
            order(),
            &text,
            &needle,
            FrenchTextSearch::ForwardFrom(0),
            &mut pool,
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}
