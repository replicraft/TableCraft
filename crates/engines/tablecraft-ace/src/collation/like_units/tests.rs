use super::*;
use sha2::{Digest, Sha256};
use std::{cell::Cell, collections::BTreeSet};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
}
fn order(version: u8) -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([9, 4, 0, version])
}

#[test]
#[ignore = "Engineering generator: emits original derived FST bytes for the checked-in regeneration script"]
fn emit_general_like_dictionaries() {
    for family in [GeneralSortFamily::Legacy, GeneralSortFamily::General2010] {
        let mut keys = BTreeSet::new();
        for unit in u16::MIN..=u16::MAX {
            keys.insert(
                key::encode_full(
                    family,
                    &[unit],
                    &mut key::Work {
                        budget: &mut budget(),
                        cancelled: &|| false,
                    },
                )
                .unwrap()
                .bytes,
            );
        }
        let set = fst::Set::from_iter(keys).unwrap();
        let bytes = set.as_fst().as_bytes();
        print!("GENERAL_LIKE_FST {family:?} ");
        for byte in bytes {
            print!("{byte:02x}");
        }
        println!();
    }
}

#[test]
fn every_raw_unit_has_an_exact_dictionary_entry_and_no_extra_keys_exist() {
    for family in [GeneralSortFamily::Legacy, GeneralSortFamily::General2010] {
        let dictionary = dictionary(family).unwrap();
        dictionary.as_fst().verify().unwrap();
        let mut expected = BTreeSet::new();
        for unit in u16::MIN..=u16::MAX {
            let key = key::encode_full(
                family,
                &[unit],
                &mut key::Work {
                    budget: &mut budget(),
                    cancelled: &|| false,
                },
            )
            .unwrap();
            assert!(
                dictionary.contains(&key.bytes),
                "{family:?} unit {unit:04x}"
            );
            expected.insert(key.bytes);
        }
        assert_eq!(dictionary.len(), expected.len());
    }
}

#[test]
fn literal_units_and_expansions_are_distinct_from_arbitrary_words_or_trimmed_spaces() {
    for version in [0, 1] {
        for text in ["a", "A", "é", "e\u{301}", "ß", "ss", "æ", "ae", " ", "\0"] {
            let units: Vec<_> = text.encode_utf16().collect();
            assert!(
                is_general_like_unit(order(version), &units, &mut budget(), &|| false).unwrap(),
                "General {version}: {text:?}"
            );
        }
        assert!(
            !is_general_like_unit(
                order(version),
                &"tablecraft".encode_utf16().collect::<Vec<_>>(),
                &mut budget(),
                &|| false
            )
            .unwrap()
        );
        assert!(
            !is_general_like_unit(
                order(version),
                &"a ".encode_utf16().collect::<Vec<_>>(),
                &mut budget(),
                &|| false
            )
            .unwrap()
        );
    }
}

#[test]
fn unknown_orders_limits_and_every_cancellation_checkpoint_fail_explicitly() {
    assert!(matches!(
        is_general_like_unit(
            NativeSortOrder::from_native_bytes([9, 4, 1, 1]),
            &[97],
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    for mut limits in [
        ResourceBudget::new(0, 0, 1000),
        ResourceBudget::new(1000, 0, 1),
    ] {
        assert!(matches!(
            is_general_like_unit(order(1), &[115, 115], &mut limits, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    is_general_like_unit(order(1), &[115, 115], &mut budget(), &|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        assert!(matches!(
            is_general_like_unit(order(1), &[115, 115], &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(InspectError::Cancelled)
        ));
    }
}

#[test]
fn derived_artifact_hashes_are_fixed() {
    for (family, expected) in [
        (
            GeneralSortFamily::Legacy,
            "5c63462ffb2a5062e53db01610e4e6440909b2ad149241024fa0bf9ea7271bf6",
        ),
        (
            GeneralSortFamily::General2010,
            "5a6d7d415a3cb502ea5d23e39e4d5db088019aedde633dcec887a57c759bbe37",
        ),
    ] {
        let dictionary = dictionary(family).unwrap();
        let actual = Sha256::digest(dictionary.as_fst().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    }
}
