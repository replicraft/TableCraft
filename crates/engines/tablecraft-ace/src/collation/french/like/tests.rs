use super::*;
use proptest::prelude::*;
use std::{cell::Cell, collections::BTreeSet};
use tablecraft_core::limits::Resource;

mod oracle;

fn order() -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([12, 4, 0, 0])
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
}

#[test]
fn every_raw_unit_is_present_and_the_french_dictionary_has_no_extra_keys() {
    let dictionary = dictionary().unwrap();
    dictionary.as_fst().verify().unwrap();
    let mut expected = BTreeSet::new();
    for unit in u16::MIN..=u16::MAX {
        let key = key::encode_full_french(
            &[unit],
            &mut key::Work {
                budget: &mut budget(),
                cancelled: &|| false,
            },
        )
        .unwrap();
        assert!(dictionary.contains(&key.bytes), "unit {unit:04x}");
        expected.insert(key.bytes);
    }
    assert_eq!(dictionary.len(), expected.len());
}

#[test]
fn pattern_keys_retain_spaces_raw_units_and_complete_reversed_accents() {
    for (left, right, expected) in [
        (vec![97, 32], vec![97], Ordering::Greater),
        (
            vec![0x00e9, 0x00e8],
            vec![0x00e8, 0x00e9],
            Ordering::Greater,
        ),
        (vec![115, 115], vec![0x00df], Ordering::Equal),
        (vec![101, 0x0301], vec![0x00e9], Ordering::Equal),
        (vec![0xd800], vec![0xd800], Ordering::Equal),
    ] {
        assert_eq!(
            compare_french_legacy_like_units(order(), &left, &right, &mut budget(), &|| false)
                .unwrap(),
            expected
        );
    }
    for text in ["a", " ", "\0", "ss", "ß", "e\u{301}", "é"] {
        let units: Vec<_> = text.encode_utf16().collect();
        assert!(
            is_french_legacy_like_unit(order(), &units, &mut budget(), &|| false).unwrap(),
            "{text:?}"
        );
    }
    assert!(!is_french_legacy_like_unit(order(), &[97, 32], &mut budget(), &|| false).unwrap());
    let mut left = vec![97; 1024];
    let mut right = left.clone();
    left.extend([0x00e9, 0x00e8]);
    right.extend([0x00e8, 0x00e9]);
    assert_eq!(
        compare_french_legacy_like_units(order(), &left, &right, &mut budget(), &|| false).unwrap(),
        Ordering::Greater
    );
}

#[test]
fn literal_and_unit_services_enforce_identity_one_short_pools_and_every_checkpoint() {
    for raw in [[9, 4, 0, 0], [12, 4, 1, 0], [12, 4, 0, 1]] {
        let unsupported = NativeSortOrder::from_native_bytes(raw);
        assert!(matches!(
            french_legacy_like_literal_step(unsupported, &[97], &[97], &mut budget(), &|| false),
            Err(InspectError::Unsupported)
        ));
        assert!(matches!(
            french_legacy_like_unit_step(unsupported, &[97], &mut budget(), &|| false),
            Err(InspectError::Unsupported)
        ));
    }
    for element in [false, true] {
        let invoke = |pool: &mut ResourceBudget, cancel: &dyn Fn() -> bool| {
            if element {
                french_legacy_like_unit_step(order(), &[101, 0x0301], pool, cancel)
            } else {
                french_legacy_like_literal_step(order(), &[101, 0x0301], &[0x00e9], pool, cancel)
            }
        };
        let mut measured = budget();
        let before_bytes = measured.remaining(Resource::Bytes);
        let before_steps = measured.remaining(Resource::Steps);
        let calls = Cell::new(0);
        let expected = invoke(&mut measured, &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(expected, LikeSpanState::Match);
        let bytes = before_bytes - measured.remaining(Resource::Bytes);
        let steps = before_steps - measured.remaining(Resource::Steps);
        for mut pool in [
            ResourceBudget::new(bytes - 1, 0, steps),
            ResourceBudget::new(bytes, 0, steps - 1),
        ] {
            assert!(matches!(
                invoke(&mut pool, &|| false),
                Err(InspectError::Limit(_))
            ));
        }
        let total = calls.get();
        for stop in 1..=total {
            calls.set(0);
            assert!(
                matches!(
                    invoke(&mut budget(), &|| {
                        calls.set(calls.get() + 1);
                        calls.get() == stop
                    }),
                    Err(InspectError::Cancelled)
                ),
                "{element}/{stop}/{total}"
            );
        }
    }
}

fn units() -> impl Strategy<Value = Vec<u16>> {
    prop::collection::vec(
        prop::sample::select(vec![
            0, 32, 97, 101, 0x00df, 0x00e9, 0x0301, 0x3005, 0x30fc, 0xd800, 0xdc00,
        ]),
        0..5,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]
    #[test]
    fn primary_impossibility_never_discards_a_later_french_match(candidate in units(), suffix in units(), literal in units()) {
        let literal_state = french_legacy_like_literal_step(order(), &candidate, &literal, &mut budget(), &|| false).unwrap();
        let element_state = french_legacy_like_unit_step(order(), &candidate, &mut budget(), &|| false).unwrap();
        if literal_state == LikeSpanState::Match {
            prop_assert_eq!(compare_french_legacy_like_units(order(), &candidate, &literal, &mut budget(), &|| false).unwrap(), Ordering::Equal);
        }
        if element_state == LikeSpanState::Match {
            prop_assert!(is_french_legacy_like_unit(order(), &candidate, &mut budget(), &|| false).unwrap());
        }
        for end in 0..=suffix.len() {
            let mut extended = candidate.clone();
            extended.extend_from_slice(&suffix[..end]);
            if literal_state == LikeSpanState::Impossible {
                prop_assert_ne!(compare_french_legacy_like_units(order(), &extended, &literal, &mut budget(), &|| false).unwrap(), Ordering::Equal);
            }
            if element_state == LikeSpanState::Impossible {
                prop_assert!(!is_french_legacy_like_unit(order(), &extended, &mut budget(), &|| false).unwrap());
            }
        }
    }
}
