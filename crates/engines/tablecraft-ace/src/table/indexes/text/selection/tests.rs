use super::*;
use crate::{NativeSortOrder, compare_general_text};
use proptest::prelude::*;
use std::cell::Cell;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 0, 1_000_000)
}

fn profile(version: u8, descending: bool) -> TextProfile {
    TextProfile {
        column: 0,
        descending,
        order: NativeSortOrder::from_native_bytes([9, 4, 0, version]),
        maximum_units: Some(255),
        selection: EncodedSelection::All,
    }
}

fn retained(profile: &TextProfile, literal: &str, candidate: &str) -> bool {
    let mut pool = budget();
    let mut work = KeyWork {
        budget: &mut pool,
        cancelled: &|| false,
    };
    let selected = EncodedSelection::new(
        TextIndexSelection::EqualityCandidates(literal),
        profile,
        &mut work,
    )
    .unwrap();
    let key = profile.encode(candidate, Some(255), &mut work).unwrap();
    assert!(!selected.contains(&[profile.null_marker()], profile.null_marker()));
    selected.contains(&key.0, profile.null_marker())
}

#[test]
fn complete_equalities_survive_case_spaces_expansions_and_key_digests() {
    for version in [0, 1] {
        for descending in [false, true] {
            let profile = profile(version, descending);
            for (literal, candidate) in [
                ("a", "A  "),
                ("é", "e\u{301}"),
                ("ß", "ss"),
                ("", "\0"),
                ("", "   "),
                ("カーカ", "カーカ "),
                ("😀", "😀 "),
                ("a-b", "a-b  "),
            ] {
                let compared =
                    compare_general_text(profile.order, literal, candidate, &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(compared, Ordering::Equal);
                assert!(
                    retained(&profile, literal, candidate),
                    "{literal:?} {candidate:?}"
                );
            }
            for text in ["a".repeat(255), "\u{6d6}".repeat(255), "😀".repeat(127)] {
                assert!(retained(&profile, &text, &text));
            }
            // Primary-prefix equality is deliberately insufficient to publish
            // a SQL match. Secondary accents and extra primary suffixes differ.
            assert!(retained(&profile, "e", "é"));
            assert!(retained(&profile, "a", "ab"));
            assert!(!retained(&profile, "a", "z"));
        }
    }
}

#[test]
fn every_scalar_bmp_unit_and_trailing_spaces_remain_candidates() {
    for version in [0, 1] {
        for code in 0..=0xffff {
            let Some(unit) = char::from_u32(code) else {
                continue;
            };
            let literal = unit.to_string();
            let candidate = format!("{literal}  ");
            for descending in [false, true] {
                assert!(
                    retained(&profile(version, descending), &literal, &candidate),
                    "{version} {code:04x}"
                );
            }
        }
    }
}

#[test]
fn prefix_fences_keep_extensions_and_both_duplicate_boundary_sides() {
    let selection = EncodedSelection::Prefix(vec![0x7f, 0x20, 0x30]);
    assert!(selection.may_match(None, None, 0));
    assert!(selection.may_match(Some(&[0x7f, 0x20, 0x30]), None, 0));
    assert!(selection.may_match(None, Some(&[0x7f, 0x20, 0x30]), 0));
    assert!(selection.may_match(
        Some(&[0x7f, 0x20, 0x30, 9]),
        Some(&[0x7f, 0x20, 0x30, 10]),
        0
    ));
    assert!(!selection.may_match(None, Some(&[0x7f, 0x20, 0x2f, 255]), 0));
    assert!(!selection.may_match(Some(&[0x7f, 0x20, 0x31]), None, 0));
    assert!(!selection.contains(&[0], 0));
}

#[test]
fn memo_unknown_orders_limits_and_cancellation_do_not_become_complete_scans() {
    let mut metadata = profile(1, false);
    metadata.maximum_units = None;
    assert!(matches!(
        EncodedSelection::new(
            TextIndexSelection::EqualityCandidates("a"),
            &metadata,
            &mut KeyWork {
                budget: &mut budget(),
                cancelled: &|| false
            }
        ),
        Err(InspectError::Unsupported)
    ));
    metadata.maximum_units = Some(255);
    metadata.order = NativeSortOrder::from_native_bytes([7, 4, 0, 1]);
    assert!(matches!(
        EncodedSelection::new(
            TextIndexSelection::EqualityCandidates("a"),
            &metadata,
            &mut KeyWork {
                budget: &mut budget(),
                cancelled: &|| false
            }
        ),
        Err(InspectError::Unsupported)
    ));

    assert_eq!(
        TextIndexSelection::EqualityCandidates(&"x".repeat(256)).validate(&mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    );

    let mut pool = ResourceBudget::new(0, 0, 1000);
    assert!(matches!(
        EncodedSelection::new(
            TextIndexSelection::EqualityCandidates("a"),
            &profile(1, false),
            &mut KeyWork {
                budget: &mut pool,
                cancelled: &|| false
            }
        ),
        Err(InspectError::Limit(_))
    ));

    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() > 5
    };
    assert!(matches!(
        EncodedSelection::new(
            TextIndexSelection::EqualityCandidates("abc"),
            &profile(1, false),
            &mut KeyWork {
                budget: &mut budget(),
                cancelled: &cancelled
            }
        ),
        Err(InspectError::Cancelled)
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn generated_strings_keep_complete_primary_weights_before_physical_digests(
        characters in prop::collection::vec(any::<char>(), 0..100),
    ) {
        let literal: String = characters.into_iter().collect();
        let candidate = format!("{literal}  ");
        for version in [0, 1] {
            for descending in [false, true] {
                prop_assert!(retained(&profile(version, descending), &literal, &candidate));
            }
        }
    }

    #[test]
    fn generated_prefix_hulls_never_discard_a_matching_key(
        prefix in prop::collection::vec(any::<u8>(), 1..34),
        suffix in prop::collection::vec(any::<u8>(), 0..64),
        cut in 0..96_usize,
    ) {
        let key = [prefix.as_slice(), suffix.as_slice()].concat();
        let lower = &key[..cut.min(key.len())];
        let upper = [key.as_slice(), &[255]].concat();
        let selection = EncodedSelection::Prefix(prefix);
        prop_assert!(selection.contains(&key, 0));
        prop_assert!(selection.may_match(Some(lower), Some(&upper), 0));
    }
}
